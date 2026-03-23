# ICFG / SDG / CPG 実装計画

このドキュメントは、現在の `ts-flow-analyzer` を ICFG / SDG / CPG へ拡張するための実装計画です。

現在の到達点:

- local metrics / predicates / effects
- intraprocedural CFG
- intra-file call graph
- shared Graph IR
- local intraprocedural def-use / data-flow

つまり「単体レイヤ」は揃っており、次は graph layer を interprocedural に接続する段階です。

---

## 1. 基本方針

### 1.1 Single-file first

最初の対象は single-file only に固定します。

- 同一ファイル内で解決できた call のみ ICFG / SDG に載せる
- import / unresolved / builtin call は Graph IR には残すが、ICFG 接続はしない

### 1.2 Graph IR は export layer のまま維持

既存設計どおり、Graph IR は thin export layer に留めます。

新しい解析本体は専用モジュールで計算し、その結果を `ir/from_*.rs` で投影します。

推奨モジュール:

- `src/icfg/`
- `src/control_dep/`
- `src/sdg/`

### 1.3 一足飛びに full CPG へ行かない

実装順は次で固定します。

1. outcome / terminal node 補強
2. ICFG
3. control dependence
4. SDG-lite
5. interprocedural data dependence
6. sparse syntax layer
7. CPG projection

---

## 2. 現状のギャップ

### 2.1 ICFG に足りないもの

現在の Graph IR は:

- `Function` / `Method` / `Class`
- `CallSite`
- `DecisionPoint`
- `CfgBlock`
- `DataFlowDef` / `DataFlowUse`

まではありますが、次がありません。

- explicit function entry / exit
- outcome / terminal node
- interprocedural cfg edge
- caller return-site node

### 2.2 SDG に足りないもの

現在すでにあるもの:

- local data dependence
- intra-procedural CFG
- call graph

まだ無いもの:

- control dependence
- formal / actual interface node
- parameter / return summary edge

### 2.3 CPG に足りないもの

現在の Graph IR は syntax layer が薄く、CPG と呼ぶには十分ではありません。

不足しているのは:

- statement / parameter / return / assignment などの syntax node
- AST child / syntax order edge
- syntax, control, data, call を束ねた layer metadata

---

## 3. フェーズ計画

## Phase 0: Graph IR terminal enrichment

### Dependency

- GitHub issue #7

### Goal

decision / CFG / SDG の終端を明示的な node で表現できるようにする。

### Changes

- `NodeKind::Outcome` を追加
- `DecisionPoint -> Outcome` の terminal edge を追加
- `return` / `throw` / happy path を outcome node へ正規化

### Why first

SDG の control dependence は「どの decision がどの terminal outcome を支配するか」を表現したくなるため、ここが先です。

---

## Phase 1: ICFG foundation

### Goal

同一ファイル内の resolved internal call を使って、function-local CFG を interprocedural に接続する。

### New module

- `src/icfg/mod.rs`
- `src/icfg/model.rs`
- `src/icfg/build.rs`
- `src/icfg/dot.rs`
- `src/ir/from_icfg.rs`

### Core model

例:

```text
IcfgReport
├── functions: Vec<IcfgFunction>
├── nodes: Vec<IcfgNode>
└── edges: Vec<IcfgEdge>
```

必要な node/edge:

- `FunctionEntry`
- `FunctionExit`
- `CallReturnSite` または caller successor anchor
- `IcfgEdge::Normal`
- `IcfgEdge::Call`
- `IcfgEdge::Return`

### Implementation approach

1. `control_flow` から function-local CFG projection を取る
2. `callgraph` の resolved internal call edge を読む
3. caller block を特定する
4. caller block -> callee entry を `Call` edge で結ぶ
5. callee exit -> caller successor block を `Return` edge で結ぶ

### Initial precision

初版は conservative over-approximation でよいです。

- block splitting はしない
- call block から複数 successor があるなら全部に return edge を張る
- import / unresolved / builtin は ICFG 接続しない

### CLI / export

- `--icfg`
- `--icfg-dot <FUNCTION>`
- `--graph --icfg`

### Acceptance criteria

- `execute -> checkRisk -> return` のような同一ファイル call path を JSON / DOT で辿れる
- `thisMethod` / `super` / `new` の resolved internal call が ICFG に反映される

---

## Phase 2: Intraprocedural control dependence

### Goal

各関数内で「どの decision がどの node を制御しているか」を表現する。

### New module

- `src/control_dep/mod.rs`
- `src/control_dep/model.rs`
- `src/control_dep/build.rs`
- `src/ir/from_control_dep.rs`

### Implementation approach

- CFG projection を使って post-dominator ベースで control dependence を計算
- decision node から:
  - cfg block
  - outcome node
  - callsite
 へ `ControlDep` を張る

### Scope

- intraprocedural only
- branch polarity は `true` / `false` を edge property で保持

### Acceptance criteria

- `if (!order) return ...` の場合、decision node から return outcome への control-dep が出る
- `if (risk) this.repo.save()` の場合、decision node から callsite / effect への control-dep が出る

---

## Phase 3: SDG-lite

### Goal

local data dependence と local control dependence を ICFG の上で束ね、single-file slicing の土台を作る。

### New module

- `src/sdg/mod.rs`
- `src/sdg/model.rs`
- `src/sdg/build.rs`
- `src/sdg/dot.rs`
- `src/ir/from_sdg.rs`

### Composition

SDG-lite は次を束ねます。

- ICFG
- intraprocedural `DataDep`
- intraprocedural `ControlDep`
- call edge

### Scope

- interprocedural data dependence はまだ入れない
- def-use は各関数内に閉じる
- ただし call / return path は ICFG で辿れる

### CLI / export

- `--sdg`
- `--sdg-dot <FUNCTION>`

### Acceptance criteria

- ある decision から、その分岐配下の local def/use と terminal outcome まで辿れる
- ある callsite から callee entry/exit まで SDG 上で辿れる

---

## Phase 4: Interface-based interprocedural data dependence

### Goal

parameter / return に限定して interprocedural data dependence を導入する。

### New nodes

- `FormalIn`
- `FormalOut`
- `ActualIn`
- `ActualOut`

### Implementation approach

1. callee 側で parameter / return summary を作る
2. caller 側で callsite ごとに actual node を作る
3. 次の edge を張る
   - `ActualIn -> FormalIn`
   - `FormalOut -> ActualOut`
   - `ActualOut -> caller-side use/return-site`

### Scope

最初は次だけ扱います。

- parameter reads
- parameter-derived return
- explicit return value

後回しにするもの:

- `this.field` write summary
- object alias
- heap update

### Acceptance criteria

- `f(x) { return x + 1 }` を `g()` から呼んだとき、caller actual から callee return 由来の edge が見える
- parameter / return 経由の data path を single-file で辿れる

---

## Phase 5: Sparse syntax layer

### Goal

CPG と呼べるだけの syntax layer を最小限追加する。

### New nodes

- `Statement`
- `Parameter`
- `Return`
- `Assignment`
- `FieldAccess`

### New edges

- `AstChild { order }` または `Syntax`

### Design rule

full AST export はしません。CFG / call / data / control に接続される node だけを syntax layer として持ちます。

### Acceptance criteria

- Graph IR 上で「この return node はどの statement / parameter / field access に対応するか」を辿れる
- syntax layer が無くても動いていた解析を壊さない

---

## Phase 6: CPG projection

### Goal

syntax + control + data + call を 1 つの export view としてまとめる。

### Output

`CPG` は別アルゴリズムではなく、各 layer をまとめた Graph IR view として出します。

含む層:

- syntax
- CFG
- ICFG
- call graph
- control dependence
- data dependence

### CLI / export

- `--cpg`
- `--cpg-dot <FUNCTION>`

### Acceptance criteria

- 1 つの graph export に syntax / control / data / call の全 edge が共存する
- node / edge kind だけで layer を区別できる

---

## 4. 推奨 issue 分割

### Existing dependency

- #7 Add outcome nodes and terminal decision edges to Graph IR

### Next issues

1. ICFG foundation over CFG + call graph
2. ICFG JSON / DOT export and Graph IR integration
3. Intraprocedural control dependence edges
4. SDG-lite composition and export
5. Formal/actual interface nodes for interprocedural data dependence
6. Sparse syntax layer for CPG
7. CPG export view

---

## 5. 非ゴール

この計画では次はやりません。

- project-wide module graph
- precise import-wide dispatch
- SSA
- alias / points-to / heap graph
- framework-specific magic の完全解決

---

## 6. 実装上の注意

- Graph IR を計算基盤にしない
- 解析本体は `icfg/`, `control_dep/`, `sdg/` の typed model を持つ
- `main.rs` では今の `from_*` パターンを維持する
- issue #7 を先に終わらせる
- single-file / resolved internal call に限定して精度より進捗を優先する

---

## 7. 参照箇所

- [`src/main.rs`](../analyzer/src/main.rs)
- [`src/control_flow/`](../analyzer/src/control_flow)
- [`src/callgraph/`](../analyzer/src/callgraph)
- [`src/dataflow/`](../analyzer/src/dataflow)
- [`src/ir/`](../analyzer/src/ir)
- [`ARCHITECTURE.md`](ARCHITECTURE.md)
- [`IMPLEMENTATION_PHASES.md`](IMPLEMENTATION_PHASES.md)

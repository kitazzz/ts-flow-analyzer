# Roadmap: ts-flow-analyzer

## Overview

TypeScript コードベースに対して静的解析を行い、関数ごとの複雑さ、条件分岐の構造、ルールエンジン化候補、さらに将来的な真理値表 / MC/DC / プロパティテスト生成までつなげる analyzer を構築する。

現在の主実装は Rust / Oxc ベースの `analyzer/` です。旧 `ts-morph` 実装は削除し、主系を Rust 側に一本化しています。

---

## Current Strategy

- 現在の主実装は Rust / Oxc ベースの `analyzer/` にある
- 解析基盤は Rust / Oxc に集中し、実験コードも原則こちらに寄せる
- 単なる complexity analyzer ではなく、最終的には
  - rule candidate detection
  - logic extraction
  - truth table generation
  - MC/DC generation
  - property-based test generation
  - LLM handoff
  まで視野に入れる
- ただし、`await repo.save(...)`, `for (...)`, `try/catch` などはそのまま論理対象にせず、**判定部分のみを抽出して論理化する**

---

## Milestone 1: Complexity Foundation

### Goal
TypeScript 関数ごとの基本複雑性を安定して計測できる CLI を作る。

### Scope
- 名前付き関数を解析対象にする
- 基本複雑度
- 関数ネスト複雑度
- JSON / pretty print 出力

### Deliverables
- 関数収集
- 基本複雑性解析
- 制御ネスト解析
- cyclomatic complexity
- 関数ネスト解析
- CLI

### Metrics
- `ifCount`
- `elseIfCount`
- `switchCount`
- `ternaryCount`
- `returnCount`
- `maxNestingDepth`
- `cyclomaticComplexity`
- `localFunctionCount`
- `functionNestingDepth`
- `callbackNestingDepth`

### Exit Criteria
- 任意の TS ファイルを解析できる
- 関数単位で上記メトリクスが出る
- 複雑な関数が直感と大きくズレず上位に来る

### Status: ✅ Complete

---

## Milestone 2: Condition & Effect Analysis

### Goal
`if` の数だけでなく、条件の複雑さと、その条件が何を引き起こすかを抽出できるようにする。

### Scope
- 条件式複雑性解析
- atomic predicate 抽出
- 条件 → effect 対応
- 副作用の粗い分類

### Deliverables
- 条件式 walker
- 条件正規化
- atomic predicate 抽出
- effect 抽出
- side effect 分類

### New Metrics / Outputs
- `negationCount`
- `atomicConditionCount`
- `maxConditionDepth`

### Effect Types
- `assignment`
- `return`
- `throw`
- `call`

### Side Effect Classification
- `dbWrite`
- `externalApi`
- `notification`
- `logging`
- `pureCall`
- `unknown`

### Exit Criteria
- 各条件式を atomic predicate に分解できる
- 各条件の先で何が起きるかを取れる
- rule 寄りの if と process 寄りの if を人間が見分けやすくなる

---

## Milestone 3: Rule Candidate & Similarity Analysis

### Goal
ルールエンジン化候補を見つけられるようにする。

### Scope
- 条件重複検出
- decision-like assignment 検出
- 出力収束解析
- tenant/company 分岐検出
- 関数間類似性解析
- 簡易 call graph

### Deliverables
- duplicate predicate 検出
- similarity analyzer
- decision target 検出
- tenant branch 検出
- rule candidate hint 出力

### Outputs
- repeated predicates
- shared targets
- similar functions
- likely rule-engine candidates

### Exit Criteria
- 条件の重複を検出できる
- 類似関数を抽出できる
- rule engine 化候補をランキングまたはヒント表示できる

---

## Milestone 4: Logic Extraction & Test Generation

### Goal
解析結果をもとに、判定部分を論理化し、真理値表・MC/DC・プロパティテスト生成につなげる。

### Scope
- predicate / decision / effect の論理正規化
- decision ごとの truth table
- MC/DC ケース生成
- 実現不能組み合わせの検出
- 入力具体化
- property-based test 雛形生成

### Deliverables
- decision logic builder
- truth table generator
- MC/DC generator
- local intraprocedural def-use / data-flow foundation
- infeasible combination detector
- input materializer
- property test renderer

### Important Rule
以下はそのまま論理対象にはしない:
- `await repo.save(...)`
- `for (...)`
- `try/catch`

判定部分のみ抽出して論理化する。

### Exit Criteria
- decision 単位で truth table を作れる
- MC/DC ケースを出せる
- 少なくとも簡易な property test 雛形を生成できる

---

## Milestone 5: LLM Handoff & Rule Design Support

### Goal
LLM が解釈しやすい形に解析結果を整え、fact / decision / action 候補を提案しやすくする。

### Scope
- YAML/JSON サマリ生成
- candidate fact / decision / action hints
- 解析しやすいコード規約の提案
- LLM 用 prompt 入力整形

### Deliverables
- YAML renderer
- LLM handoff schema
- candidate naming hints
- analyzer / lint ルール候補

### Exit Criteria
- 解析結果を LLM に渡して意味づけしやすい
- fact / decision 候補の命名補助ができる
- 書き方の改善提案までつながる

---

## Implementation Track: ICFG / SDG / CPG

### Goal
現在の single-file analyzer を、program graph 系の基盤へ段階的に拡張する。

対象は一足飛びの full CPG ではなく、次の順で積み上げること。

1. ICFG（interprocedural control-flow graph）
2. SDG-lite（ICFG + intraprocedural control/data dependence）
3. interface node を伴う interprocedural data dependence
4. sparse syntax layer を含む CPG view

### Dependency
- issue #7: Graph IR の outcome node / terminal edge を先に入れる

### Scope
- single-file only
- resolved internal call のみ ICFG / SDG 接続
- `thisMethod` / `super` / `new` は既存 call graph 解決を再利用
- `await` は通常 call と同じ扱い

### Non-goals in This Track
- project-wide / precise interprocedural analysis
- SSA 変換
- alias / points-to / heap modeling
- import 越しの正確な dispatch

### Phase G1: ICFG foundation
- explicit `FunctionEntry` / `FunctionExit` / `Outcome` ノードを導入
- CFG と call graph をつなぐ内部 `IcfgReport` モデルを追加
- caller block -> callee entry、callee exit -> caller successor block の conservative 接続を行う
- `--icfg` / `--icfg-dot` を追加する

### Phase G2: SDG-lite
- intraprocedural `ControlDep` を導入
- 既存 `DataDep` と組み合わせて function-local dependence layer を作る
- `--sdg` / `--sdg-dot` で export する

### Phase G3: Interprocedural data dependence
- `FormalIn` / `FormalOut` / `ActualIn` / `ActualOut` interface node を導入
- parameter / return のみを対象に summary edge を作る
- `this.field` や heap mutation は後続に分離する

### Phase G4: CPG projection
- sparse syntax layer を追加する
- AST / CFG / ICFG / call / control-dep / data-dep を 1 つの Graph view として束ねる
- `--cpg` export を追加する

### Exit Criteria
- 同一ファイル内の caller -> callee -> return path を ICFG として辿れる
- decision / outcome / effect / data-flow を SDG 上で関連付けられる
- syntax + control + data + call を CPG view として 1 つの export に載せられる

---

## Design Principles

### 1. 判定と副作用を分ける
複雑条件と `save / send / publish` が同じ block に混在している場合、将来的には分離を促す。

### 2. guard clause と business rule を分ける
- null check
- technical fallback
- environment branch

と

- approval decision
- accounting classification
- tenant-specific rule

は区別して扱う。

### 3. 関数全体を論理化しない
関数全体を真理値表化するのではなく、**decision / predicate / effect** に分解して扱う。

### 4. Rust / Oxc を主系として伸ばす
現在の優先は Rust / Oxc 実装を前提に次を積み上げること。
- intraprocedural CFG の強化
- local def-use / data-flow の活用
- function summary / interprocedural 解析
- Graph IR を SDG / CPG へ拡張
- analyzer core の独立性と実行性能の向上

---

## Near-term Priority

1. issue #7 で outcome node / terminal edge を入れる
2. G1 で single-file ICFG を入れる
3. G2 で SDG-lite を入れる
4. G3 で parameter / return summary edge を入れる
5. G4 で sparse CPG view を入れる

---

## Non-goals for Now

現時点では以下はやらない:
- project-wide / precise interprocedural analysis
- SSA / alias / heap modeling
- framework magic の完全解決
- 全関数の完全自動 rule engine 変換

---

## Success Definition

このプロジェクトの短中期的な成功は次の通り。

- 複雑な関数を定量的に見つけられる
- 条件と効果を抽出できる
- rule engine 化候補を説明付きで提示できる
- decision 単位で truth table / MC/DC / property-based test へつなげられる
- LLM と人間の両方が読みやすい解析結果を出せる

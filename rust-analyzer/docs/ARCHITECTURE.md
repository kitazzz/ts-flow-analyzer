# recast-forge-analyzer 設計書

## 1. プロジェクト概要

`recast-forge-analyzer`（バイナリ名: `rf-analyze`）は、TypeScript / JavaScript ソースコードを対象にした Rust 製の静的解析 CLI です。

実装を読む順番とフェーズ別のコード導線は [`IMPLEMENTATION_PHASES.md`](IMPLEMENTATION_PHASES.md) を参照してください。

**目的**: 関数・メソッド単位で「何を判断し、何をするか」を構造的に抽出する。
テストケース設計、コードレビュー、LLM 連携のための中間表現（IR）生成を視野に入れた解析基盤です。

**解析対象**: 単一ファイル内の関数・メソッド・クラス。

**非目的**: コンパイラ、リンター、フォーマッタではありません。型チェックやエラー検出は行いません。

---

## 2. 設計思想

### 2.1 Single-File First

プロジェクト全体の型解決やモジュールグラフの構築はスコープ外です。1 ファイルの中で完結する解析を深く正確に行うことを優先しています。マルチファイル解決は将来フェーズとして設計されています。

### 2.2 段階的解析パイプライン

解析は独立した段階に分かれています。各段階は前段の出力のみを入力とし、相互に依存しません。

```
Source → Parse → Collect Functions
       → [Metrics | Predicates | Effects | Data Flow | Decision Table]
       → Call Graph
       → Graph IR (functions + call graph + decision points + data flow + optional CFG)
```

ユーザーは CLI フラグで必要な解析だけを有効化できます。これにより不要な計算を省き、出力ノイズを抑えます。

### 2.3 AST 直接操作

Oxc パーサーの AST ノードを直接走査して情報を抽出します。正規表現やテキストパターンマッチではなく、構文木の構造に基づいた解析を行います。

ソーステキストへのアクセスは `source[span.start..span.end]` で行います。Oxc の `Span` はバイトオフセットを保持しており、行番号は改行文字のカウントで算出します。

### 2.4 Opt-in 拡張

CFG（制御フローグラフ）解析は `cfg-analysis` feature gate の背後にあります。これは `oxc_semantic` と `oxc_cfg` への依存を追加するため、必要な場合のみビルドに含めます。

---

## 3. アーキテクチャ概観

### 3.1 処理フロー

```
┌─────────────┐
│  CLI 解析    │  cli/mod.rs
│  (clap)     │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ ソース読み込み│  parser/load_source.rs
│  (fs::read)  │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Oxc Parse   │  oxc_parser::Parser
│  → Program   │
└──────┬──────┘
       │
       ▼
┌─────────────────┐
│ 関数収集         │  ast/collect_functions.rs
│ → Vec<Collected  │
│   Function>      │
└──────┬──────────┘
       │
       ├──────────────────────────────────────────────┐
       │                                              │
       ▼                                              ▼
┌─────────────────┐                          ┌──────────────────┐
│ 各関数に対して    │                          │ ファイル全体に対して │
│ (for func in ..) │                          │                  │
├─────────────────┤                          ├──────────────────┤
│ Metrics          │ metrics/                 │ Call Graph       │ callgraph/
│ Predicates       │ predicates/              │ (nodes, edges,   │
│ Effects          │ effects/                 │  imports)         │
│ Data Flow        │ dataflow/                │                  │
│ Decision Table   │ decision/                ├──────────────────┤
└──────┬──────────┘                          │ Graph IR         │ ir/
       │                                     │ (nodes, edges)   │
       ▼                                     └──────────────────┘
┌─────────────────┐
│ JSON / テキスト   │  main.rs (出力部)
│ / DOT 出力       │
└─────────────────┘
```

### 3.2 モジュール一覧

```
src/
├── main.rs                    # パイプライン制御、出力
├── cli/mod.rs                 # CLI 定義（clap）
├── parser/
│   └── load_source.rs         # ファイル読み込み
├── ast/
│   └── collect_functions.rs   # 関数・クラス・メソッド収集
├── model/
│   └── mod.rs                 # 全データ型定義
├── metrics/
│   ├── complexity.rs          # 基本カウント + 条件分解
│   ├── cyclomatic.rs          # サイクロマティック複雑度
│   ├── nesting.rs             # 最大ネスト深さ
│   └── function_nesting.rs    # ローカル関数 / コールバック深さ
├── predicates/
│   ├── extract.rs             # 条件式抽出
│   ├── classify.rs            # 条件式分類
│   └── normalize.rs           # 意味命名 / 正規化
├── effects/
│   └── extract.rs             # 副作用抽出 + 分類
├── dataflow/
│   ├── model.rs               # Def / Use / DefUseEdge / DataFlowReport
│   └── analyze.rs             # intraprocedural def-use / reaching-def walker
├── decision/
│   ├── model.rs               # パス状態の型定義
│   ├── table.rs               # Decision table 構築
│   └── mcdc.rs                # MC/DC ペア選択
├── control_flow/              # [cfg-analysis feature]
│   ├── context.rs             # Semantic + CFG コンテキスト
│   ├── dot.rs                 # CFG DOT レンダリング
│   └── reachability.rs        # 到達可能性判定
├── callgraph/
│   ├── mod.rs                 # Call graph 構築オーケストレータ
│   ├── model.rs               # CallEdge, CallGraphNode 等
│   ├── collect_calls.rs       # AST から CallSite 抽出
│   ├── collect_imports.rs     # import 宣言抽出
│   ├── resolve.rs             # 呼び出し先解決
│   ├── classify.rs            # domain / infra / builtin 分類
│   └── dot.rs                 # Call graph DOT レンダリング
└── ir/
    ├── graph.rs               # GraphIR, GraphNode, GraphEdge
    ├── builder.rs             # GraphBuilder
    ├── from_functions.rs      # CollectedFunction → Function/Class/Method ノード
    ├── from_callgraph.rs      # CallGraphData → CallSite / ExternalSymbol / Call edge
    ├── from_data_flow.rs      # DataFlowReport → DataFlowDef / DataFlowUse / DataDep
    ├── from_decision.rs       # DecisionTableData → DecisionPoint / DecisionBranch
    ├── from_cfg.rs            # CFG → CfgBlock / Cfg edge [cfg-analysis feature]
    ├── dot.rs                 # Graph IR DOT レンダリング
    └── function_report.rs     # LLM 向け report フォーマッタ（簡易）
```

---

## 4. 核心概念

### 4.1 CollectedFunction — 解析単位

解析の基本単位は `CollectedFunction` です。ファイル内の以下を収集します:

| 対象 | SymbolKind | symbol_name の例 |
|------|-----------|-----------------|
| `function foo()` | Function | `foo` |
| `const bar = () => {}` | VariableFunction | `bar` |
| `class Svc {}` | Class | `Svc` |
| `class Svc { execute() {} }` | Method | `Svc#execute` |

**Class ノードの特殊性**: Class 自体は `CollectedFunction` として登録されますが、statement body を持たないため、metrics は最小値になります。call graph のノードからは除外されます。

### 4.2 FunctionReport — 関数レポート

各関数に対して生成される解析結果の集約です。

```
FunctionReport
├── symbol_name: "ApproveOrderUseCase#execute"
├── symbol_kind: Method
├── class_name: Some("ApproveOrderUseCase")
├── member_name: Some("execute")
├── function_name: "execute"
├── file_path: "../samples/usecase/approveOrder.ts"
├── start_line: 26
├── metrics: FunctionMetrics { ... }
├── predicates: Option<Vec<AtomicPredicate>>    ← --predicates / --all
├── effects: Option<Vec<Effect>>                ← --effects / --all
├── data_flow: Option<DataFlowReport>           ← --data-flow / --all
└── decision_table: Option<DecisionTableData>   ← --decision / --all
```

Optional フィールドは CLI フラグに応じて生成されます。`skip_serializing_if = "Option::is_none"` により、JSON 出力では不要なフィールドが省略されます。

### 4.3 AtomicPredicate — 原子的条件

条件式を論理演算子で分解した最小単位です。

```ts
if (operator.role !== 'admin' && operator.role !== 'vip') { ... }
```

は 2 つの AtomicPredicate に分解されます:

| text | negated | kind | normalizedName |
|------|---------|------|---------------|
| `operator.role !== 'admin'` | false | Comparison | `operatorRoleNotAdmin` |
| `operator.role !== 'vip'` | false | Comparison | `operatorRoleNotVip` |

**分解規則**: `&&`、`||`、`??` で分割し、`!` は negated フラグに吸収します。括弧は展開されます。

### 4.4 正規化名 — 3 カテゴリ

truthiness 系の条件は、元式の意味に応じて 3 つのカテゴリで命名されます。

#### A. nullable/object 系

変数そのものの存在を検査している場合。

```ts
if (!order) → orderMissing
if (operator) → operatorExists
```

#### B. boolean property 系

ドット区切りの末尾がブール値を示す場合。

```ts
if (!riskCheck.ok) → riskCheckNotOk
if (!input.forceApprove) → inputForceApproveDisabled
```

判定基準:
- 既知のブール名: `ok`, `valid`, `enabled`, `active`, `approved`, `success` 等
- ブール接頭辞: `is*`, `has*`, `can*`, `should*` 等
- アクション接頭辞: `force*`, `skip*`, `allow*`, `prevent*`, `ignore*` 等

#### C. already-meaningful boolean 系

変数名自体がブール値の意味を持つ場合、そのまま使います。

```ts
if (hasZeroPriceItem) → hasZeroPriceItem      // Exists を付けない
if (!isAdmin)         → isNotAdmin             // Missing ではなく意味反転
```

**否定の変換規則**:

| 接頭辞 | 肯定 → 否定 |
|-------|-----------|
| `has*` | `hasItems` → `noItems` |
| `is*` | `isValid` → `isNotValid` |
| `can*` | `canEdit` → `cannotEdit` |
| `ok` | `riskCheckOk` → `riskCheckNotOk` |
| `force*` 等 | `forceApprove` → `forceApproveDisabled` |

### 4.5 Effect — 副作用

関数内の副作用を `kind`（何が起きるか）と `sideEffect`（どの領域に影響するか）で分類します。

| kind | 検出元 |
|------|-------|
| Return | `return` 文 |
| Throw | `throw` 文 |
| Assignment | `x = y` 代入式 |
| Call | 関数呼び出し / `await` 式 |

| sideEffect | 判定キーワード |
|-----------|-------------|
| DbRead | find, get, query, fetch, load, select, search, list |
| DbWrite | save, update, delete, remove, insert, upsert, create, put, patch |
| ExternalApi | http, axios, request, post, send, publish, emit, notify, dispatch |
| Logging | log, warn, debug, console, logger, trace |
| StateWrite | 代入式全般 |
| PureCall | 上記に該当しない呼び出し |

キーワードマッチは `matches_keyword()` で行い、識別子の途中にあるサブストリングにはマッチしません（単語境界チェック + camelCase 境界認識）。

### 4.6 Decision Table — 判定テーブル

関数内のすべての if 文を順に追跡し、各実行パスの条件組み合わせと結果を真理値表として出力します。

```
decisions:  D0=!order  D1=!operator  D2=role&&  D3=!canTransition  D4=!forceApprove  D5=!riskCheck.ok
```

| D0 | D1 | D2 | D3 | D4 | D5 | outcome |
|----|----|----|----|----|----|----|
| T  | *  | *  | *  | *  | *  | return { ok: false, error: 'Order not found' } |
| F  | T  | *  | *  | *  | *  | return { ok: false, error: 'Operator not found' } |
| F  | F  | T  | *  | *  | *  | return { ok: false, error: 'Insufficient ...' } |
| F  | F  | F  | T  | *  | *  | return { ok: false, error: 'Cannot ...' } |
| F  | F  | F  | F  | T  | T  | return { ok: false, error: 'Risk ...' } |
| F  | F  | F  | F  | F  | *  | return { ok: true, order: saved } |
| F  | F  | F  | F  | T  | F  | return { ok: true, order: saved } |

`*` は「この条件が評価されない」ことを示します（前段の early return で到達しない、または short-circuit で評価されない）。

**happy_path**: 成功パス（`ok: true` を返す行）のうち、最も具体的（`*` が少ない）なものが選ばれます。

#### 拡張モード（--decision-enhanced）

`&&` / `||` を短絡評価を考慮して複数列に展開します。

```ts
if (a && b) { ... }
```

通常モード: 1 列（`a && b` を 1 decision として扱う）
拡張モード: 2 列（D_a, D_b）

| D_a | D_b | 結果 |
|-----|-----|------|
| F   | *   | else 分岐（短絡: b は評価されない）|
| T   | F   | else 分岐 |
| T   | T   | then 分岐 |

`??`（nullish coalescing）は boolean 短絡ではないため展開対象外です。

### 4.7 MC/DC ペア

各 decision point に対して、T→F / F→T の出力を持つ行のペアを選びます。

選択基準:
1. 対象 decision の値が T と F で異なる
2. 他の decision の値がなるべく一致する
3. CFG で到達不能とマークされた行は除外する
4. happy path を含むペアにはボーナススコアを付与する

### 4.8 Data Flow — local def-use / reaching-def

`--data-flow` は、単一関数・単一ファイル・intraprocedural な def-use 解析を返します。

```
DataFlowReport
├── defs: Vec<Def>
├── uses: Vec<Use>
└── def_use_edges: Vec<DefUseEdge>
```

#### DefKind

| kind | 例 |
|------|---|
| `Declaration` | `const x = ...` |
| `Parameter` | `function f(x)` |
| `Assignment` | `x = y`, `x += y`, `i++` |
| `ForBinding` | `for (let x of xs)` / `for (x in obj)` |
| `CatchBinding` | `catch (e)` |
| `Destructuring` | `const { a } = obj`, `({ a } = obj)` |

#### UseKind

| kind | 例 |
|------|---|
| `Read` | `return x` |
| `MemberRead` | `return this.value` |

#### セマンティクス

- lexical scope と function scope を `BindingId` で分離して tracking
- `var` は prepass で hoist
- body-level `function declaration` は hoist
- `this.field` は field ごとに別 binding として tracking
- loop は dry-run + real-walk の 2 段階で reaching set を安定化
- `mayReach = true` は「その def が一部分岐にしか存在しない」ことを表す

初期化なし宣言は `defs` には入りますが、reaching def には入りません。

### 4.9 Call Graph — 呼び出しグラフ

ファイル内の関数間呼び出し関係を表現します。

```
CallGraphData
├── nodes: ファイル内の全関数/メソッド（Class ノード除外）
├── edges: 各関数から呼ばれる call site ごとに 1 エッジ
│   └── line / span_start / span_end を保持
└── imports: ファイル先頭の import 宣言（type-only 除外）
```

#### CallKind — 呼び出しの形態

| kind | 例 | callee 解決 |
|------|---|-----------|
| Direct | `canTransition()` | 同一ファイル関数 or import |
| ThisMethod | `this.checkRisk()` | 同一クラスまたは親クラスのメソッド |
| MemberCall | `this.orderRepo.findById()` | 未解決 |
| Super | `super.method()` | 親クラスチェーン上のメソッド |
| SuperConstructor | `super()` | 親クラス constructor |
| New | `new Foo()` | 同一ファイル class / function または import |

#### CallCategory — 呼び出しの分類

| category | 判定基準 | デフォルト表示 |
|---------|---------|-------------|
| Domain | 内部関数 or import に解決済み | 表示 |
| Infra | receiver が repo/client/db 系 | 表示 |
| Builtin | `reduce`, `some`, `toString` 等 | **非表示** |
| Unresolved | 上記いずれにも該当しない | 表示 |

Builtin は `--include-builtin-calls` で表示できます。

#### 解決ルール（優先順）

1. **ThisMethod**: `this.method()` → 現在クラスから親クラスへ向かって `ClassName#method` を検索
2. **Direct（ファイル内）**: `foo()` → class_name が None の CollectedFunction から検索
3. **Direct（import）**: `foo()` → import map から検索
4. **Super**: 親クラスから上方向に `Parent#method` を検索
5. **SuperConstructor**: 親クラスの `#constructor` を検索
6. **New**: class / function / import を検索
7. **MemberCall**: 未解決（receiver chain を記録）

#### DOT レンダリング

entrypoint を指定すると、BFS で到達可能なノードだけを描画します。

ノード色:
- 黄色（実線）: ファイル内関数
- 水色（破線）: import 経由の外部関数
- 赤色（点線）: 未解決の呼び出し先

### 4.10 Graph IR — 統一グラフ表現

`--graph` は、既存の解析結果を単一の node/edge モデルに合成した `GraphIR` を返します。

```
GraphIR
├── file_path
├── nodes: Vec<GraphNode>
└── edges: Vec<GraphEdge>
```

#### NodeKind

| kind | 意味 |
|------|------|
| Function | top-level function / variable function |
| Class | class declaration |
| Method | class method |
| CallSite | 呼び出し位置 |
| ExternalSymbol | import / unresolved call target |
| DecisionPoint | decision table の predicate |
| DataFlowDef | def site |
| DataFlowUse | use site |
| CfgBlock | CFG の basic block（`cfg-analysis` 時のみ） |

#### EdgeKind

| type | 意味 |
|------|------|
| Contains | 構造上の所属関係（Class→Method, Function→CallSite 等） |
| Call | CallSite→callee |
| DecisionBranch | DecisionPoint 間の分岐サマリ |
| DataDep | DataFlowDef→DataFlowUse |
| Cfg | CFG block 間遷移 |

#### 現在の構築方針

- `Function` / `Method` / `Class` は `CollectedFunction` から生成
- `CallSite` は call graph から生成し、call expression の `line` / `span` を保持
- `ExternalSymbol` は import または unresolved target を first-class node として持つ
- `DecisionPoint` は decision table の predicate から生成し、truth row を使って summary `DecisionBranch` を張る
- `DataFlowDef` / `DataFlowUse` は `DataFlowReport` から生成し、`DataDep` edge を張る
- `CfgBlock` は `cfg-analysis` 付きビルド時のみ生成

**制約**:

- `--graph` は常に file-scope です。`--function` は `functions` 配列だけを絞り、`graph` 自体は絞りません
- decision graph はまだ outcome node まで含む完全 DAG ではありません。terminal outcome を含む拡張は follow-up issue で管理します

### 4.11 メトリクス — 14 指標

| 指標 | 説明 | 算出方法 |
|-----|------|---------|
| `ifCount` | if 文の数 | AST 走査 |
| `elseIfCount` | else-if の数 | alternate が IfStatement |
| `switchCount` | switch 文の数 | AST 走査 |
| `ternaryCount` | 三項演算子の数 | ConditionalExpression |
| `returnCount` | return 文の数 | AST 走査 |
| `maxNestingDepth` | 制御構造の最大ネスト深さ | if/switch/loop/try で +1 |
| `cyclomaticComplexity` | サイクロマティック複雑度 | 1 + if + case + ternary |
| `localFunctionCount` | ローカル関数宣言の数 | 関数/アロー式のカウント |
| `functionNestingDepth` | 関数宣言のネスト深さ | 入れ子の function 宣言 |
| `callbackNestingDepth` | コールバックネスト深さ | 引数位置の関数ネスト |
| `logicalOperatorCount` | `&&` / `||` / `??` の数 | LogicalExpression |
| `negationCount` | `!` の数 | UnaryExpression(LogicalNot) |
| `atomicConditionCount` | 原子的条件の数 | 条件式の末端ノード数 |
| `maxConditionDepth` | 条件式の最大ネスト深さ | `&&` / `||` のネスト段数 |

**atomicConditionCount の例**:

```ts
if (a && b)           // → 2 atomics
if (a)                // → 1 atomic
if (a || (b && c))    // → 3 atomics
```

**maxConditionDepth の例**:

```ts
if (a && b)           // → depth 1
if (a)                // → depth 0
if (a || (b && c))    // → depth 2（|| の中に && がネスト）
```

---

## 5. JSON 出力仕様

### 5.1 通常モード

`--json` のみ（`--call-graph` / `--all` なし）の場合、トップレベルは `FunctionReport[]` 配列です。

```json
[
  {
    "symbolName": "ApproveOrderUseCase#execute",
    "symbolKind": "method",
    "className": "ApproveOrderUseCase",
    "memberName": "execute",
    "functionName": "execute",
    "filePath": "...",
    "startLine": 26,
    "metrics": { ... }
  }
]
```

### 5.2 オブジェクト形式モード

`--call-graph`、`--all`、または `--graph` を指定すると、トップレベルがオブジェクトになります。

```json
{
  "functions": [ /* FunctionReport[] */ ],
  "callGraph": {
    "nodes": [ /* CallGraphNode[] */ ],
    "edges": [ /* CallEdge[] */ ],
    "imports": [ /* ImportEntry[] */ ]
  },
  "graph": {
    "filePath": "path/to/file.ts",
    "nodes": [ /* GraphNode[] */ ],
    "edges": [ /* GraphEdge[] */ ]
  }
}
```

`callGraph` は `--call-graph` / `--all` のときだけ、`graph` は `--graph` のときだけ含まれます。
`functions[].dataFlow` は `--data-flow` / `--all` のときだけ含まれます。

**後方互換性**: これらのフラグなしでは従来の配列形式を維持します。

### 5.3 DOT 出力モード

`--cfg-dot <FUNCTION>`、`--call-graph-dot <FUNCTION>`、`--graph-dot <FUNCTION>` は JSON ではなく DOT テキストを stdout に出力して終了します。

- `--cfg-dot`: 指定 function / method の CFG 部分グラフ
- `--call-graph-dot`: 指定 entrypoint から到達可能な call graph
- `--graph-dot`: 指定 function / method を root にした Graph IR 部分グラフ

---

## 6. AST 走査パターン

全モジュールで共通する走査パターンがあります。

### 6.1 Statement 再帰走査

```rust
fn walk_stmt(stmt: &Statement<'_>, source: &str, out: &mut Vec<T>) {
    match stmt {
        Statement::IfStatement(s) => { /* test を処理 + consequent/alternate を再帰 */ }
        Statement::BlockStatement(b) => { /* body を再帰 */ }
        Statement::WhileStatement(w) => { /* test + body */ }
        Statement::TryStatement(t) => { /* block + handler + finalizer */ }
        Statement::SwitchStatement(sw) => { /* cases を再帰 */ }
        Statement::FunctionDeclaration(_) => { /* 入れ子関数には入らない */ }
        _ => {}
    }
}
```

**重要な規約**: ネストした関数宣言（`FunctionDeclaration`）やアロー関数式（`ArrowFunctionExpression`）の中には再帰しません。これらは別の `CollectedFunction` として独立に解析されます。

### 6.2 ソーステキスト取得

```rust
let text = &source[node.span().start as usize..node.span().end as usize];
```

Oxc の `Span` はバイトオフセットです。UTF-8 境界に注意する必要がありますが、JavaScript ソースでは通常問題になりません。

### 6.3 行番号算出

```rust
fn span_line(source: &str, start: u32) -> u32 {
    source[..start as usize].bytes().filter(|b| *b == b'\n').count() as u32 + 1
}
```

ソース先頭から指定バイト位置までの改行数 + 1 = 行番号。この関数は複数モジュールに重複定義されています（意図的: モジュール間の依存を避けるため）。

---

## 7. Feature Gate: cfg-analysis

### 7.1 依存関係

```toml
[features]
cfg-analysis = ["dep:oxc_semantic", "oxc_semantic/cfg", "dep:oxc_cfg"]
```

`oxc_semantic` は AST に対して意味解析（バインディング、スコープ、CFG）を行います。`oxc_cfg` は制御フローグラフの型定義です。

### 7.2 有効化される機能

- `--cfg-dot <FUNCTION>`: 関数の CFG を DOT で可視化
- `--decision-enhanced` の `terminalReachable` フィールド: 各 truth row の到達可能性
- `--data-flow`: 関数ごとの local def-use / reaching-def レポート
- `--graph` / `--graph-dot` に `CfgBlock` ノードと `Cfg` edge を含める
- `--graph` / `--graph-dot` に `DataFlowDef` / `DataFlowUse` / `DataDep` を含める（`--data-flow` / `--all` 時）
- 内部的に `CfgContext` が構築され、reachability 判定に使用される

### 7.3 無効時の振る舞い

`cfg-analysis` なしでも全機能が利用可能です（CFG 関連のみ制限）。

- `--cfg-dot` はエラーメッセージを出して終了
- `--decision-enhanced` は `&&` / `||` 展開のみ動作し、`terminalReachable` は省略
- `--graph` / `--graph-dot` は動作するが、`CfgBlock` / `Cfg` edge は含まれない

---

## 8. 依存ライブラリ

| クレート | バージョン | 用途 |
|---------|----------|------|
| `oxc_allocator` | 0.121 | Oxc のアリーナアロケータ |
| `oxc_parser` | 0.121 | JavaScript / TypeScript パーサー |
| `oxc_ast` | 0.121 | AST 型定義 |
| `oxc_span` | 0.121 | ソース位置（Span, GetSpan trait）|
| `oxc_semantic` | 0.121 (opt) | 意味解析 + CFG 構築 |
| `oxc_cfg` | 0.121 (opt) | CFG 型定義 |
| `serde` | 1 | シリアライズ / デシリアライズ |
| `serde_json` | 1 | JSON 出力 |
| `clap` | 4 | CLI 引数パース |

Oxc 0.121 は AST 型として以下を使用:
- `CallExpression`, `StaticMemberExpression`, `ComputedMemberExpression`
- `ImportDeclaration`, `ImportSpecifier`, `ModuleExportName`
- `LogicalExpression`（`&&`, `||`, `??`）
- `UnaryExpression`（`!`, `typeof`）
- `BinaryExpression`（比較演算子全般）

---

## 9. 実装マイルストーン

| Phase | 内容 | 状態 |
|-------|------|------|
| O1 | Rust + Oxc 基盤構築、基本メトリクス | 完了 |
| O2 | サイクロマティック複雑度、ネスト深さ | 完了 |
| O3 | Predicate 抽出、分類 | 完了 |
| O4 | Effect 抽出、副作用分類 | 完了 |
| O5 | Decision table、MC/DC ペア | 完了 |
| O6 | CFG 統合、短絡展開、到達可能性 | 完了 |
| O7 | Intra-file call graph | 完了 |
| O7+ | Predicate 正規化改善、条件分解強化、call graph 分類 | 完了 |
| O8 | Graph IR foundation (`--graph`, `--graph-dot`) | 完了 |
| O8.5 | Local def-use / data-flow foundation | 完了 |
| O9 | LLM 向け IR / decision DAG 拡張 | 未着手 |

### 今後の拡張候補

- マルチファイルモジュール解決（import 先の解析）
- コンストラクタインジェクション追跡（`this.field` の型解決）
- `oxc_semantic` ベースのバインディング解決
- コールバック / 高階関数の追跡
- Graph IR 上での complete decision DAG（outcome node / terminal edge）
- Graph IR 上での interprocedural data-dep / SDG・CPG 拡張
- Outcome label の安定化（エラーメッセージからの自動命名）
- Effects と guard path の紐付け（path-aware effects）
- CFG ベースの strict MC/DC

---

## 10. ディレクトリ構成とデータフロー図

```
samples/usecase/approveOrder.ts
        │
        ▼
┌──────────────────────────────────────────────────────────┐
│                        main.rs                            │
│                                                          │
│  1. CLI 解析 ─────────────────────────────────────┐      │
│  2. ソース読み込み                                   │      │
│  3. Oxc パース → Program                            │      │
│  4. collect_functions() → Vec<CollectedFunction>    │      │
│                                                    │      │
│  ┌──── 各関数ループ ─────────────────────┐          │      │
│  │                                      │          │      │
│  │  metrics/                            │          │      │
│  │  ├─ complexity   → if, switch, ...   │          │      │
│  │  ├─ cyclomatic   → CC                │          │      │
│  │  ├─ nesting      → max depth         │          │      │
│  │  └─ fn_nesting   → local fn, cb      │          │      │
│  │                                      │          │      │
│  │  predicates/                         │          │      │
│  │  ├─ extract      → AtomicPredicate[] │          │      │
│  │  ├─ classify     → PredicateKind     │  CLI     │      │
│  │  └─ normalize    → normalizedName    │  flags   │      │
│  │                                      │  に応じて │      │
│  │  effects/                            │  有効化   │      │
│  │  └─ extract      → Effect[]          │          │      │
│  │                                      │          │      │
│  │  decision/                           │          │      │
│  │  ├─ table        → DecisionTableData │          │      │
│  │  └─ mcdc         → McdcCase[]        │          │      │
│  │                                      │          │      │
│  │  → FunctionReport                    │          │      │
│  └──────────────────────────────────────┘          │      │
│                                                    │      │
│  callgraph/ (ファイル全体)                          │      │
│  ├─ collect_imports  → ImportEntry[]                │      │
│  ├─ collect_calls    → CallSite[] (per function)    │      │
│  ├─ resolve          → ResolvedTarget               │      │
│  ├─ classify         → CallCategory                 │      │
│  └─ → CallGraphData                                 │      │
│                                                    │      │
│  ir/ (ファイル全体)                                 │      │
│  ├─ from_functions  → Function/Class/Method nodes   │      │
│  ├─ from_callgraph  → CallSite/ExternalSymbol nodes │      │
│  ├─ from_decision   → DecisionPoint/Branch edges    │      │
│  ├─ from_cfg        → CfgBlock/Cfg edges            │      │
│  └─ → GraphIR                                        │      │
│                                                    │      │
│  5. JSON / テキスト出力 ◀─────────────────────────┘      │
└──────────────────────────────────────────────────────────┘
```

---

## 11. サンプルによる解析例

`samples/usecase/approveOrder.ts` を入力とした場合の出力概要:

### 収集される関数

| symbol_name | kind | start_line |
|------------|------|-----------|
| `ApproveOrderUseCase` | Class | 20 |
| `ApproveOrderUseCase#execute` | Method | 26 |
| `ApproveOrderUseCase#checkRisk` | Method | 56 |

### execute のメトリクス

| 指標 | 値 |
|-----|---|
| cyclomaticComplexity | 7 |
| ifCount | 6 |
| maxNestingDepth | 2 |
| logicalOperatorCount | 1 |
| atomicConditionCount | 7 |
| maxConditionDepth | 1 |

### execute の正規化済み predicates

| text | normalizedName |
|------|---------------|
| `order` (negated) | `orderMissing` |
| `operator` (negated) | `operatorMissing` |
| `operator.role !== 'admin'` | `operatorRoleNotAdmin` |
| `operator.role !== 'vip'` | `operatorRoleNotVip` |
| `canTransition(order.status, 'approved')` (negated) | `canTransitionApprovedFailed` |
| `input.forceApprove` (negated) | `inputForceApproveDisabled` |
| `riskCheck.ok` (negated) | `riskCheckNotOk` |

### execute の call graph edges（デフォルト: builtin 非表示）

| caller | targetName | kind | category |
|--------|-----------|------|---------|
| execute | findById | MemberCall | Infra |
| execute | findById | MemberCall | Infra |
| execute | canTransition | Direct | Domain |
| execute | checkRisk | ThisMethod | Domain |
| execute | transitionOrder | Direct | Domain |
| execute | save | MemberCall | Infra |

---

## 12. 用語集

| 用語 | 意味 |
|-----|------|
| **Atomic Predicate** | 論理演算子で分解できない最小の条件式 |
| **Composed Decision** | `&&` / `||` で結合された複合条件 |
| **Decision Point** | if / while / for / ternary の条件位置 |
| **Truth Row** | Decision table の 1 行。全条件の T/F/* と結果 |
| **Happy Path** | 正常系（成功を返す）のパス |
| **Early Return** | ガード節による早期 return |
| **Short-circuit** | `&&` の左が false なら右を評価しない（`||` は逆）|
| **MC/DC** | Modified Condition/Decision Coverage。各条件が独立に結果を変えることを示すペア |
| **CallSite** | 関数本体内の 1 つの呼び出し箇所 |
| **CallEdge** | caller → callee の解決済み/未解決の辺 |
| **Builtin** | `Array.prototype.reduce` 等の組み込みメソッド |
| **Infra** | repository / client 等のインフラ層呼び出し |
| **CFG** | Control Flow Graph（制御フローグラフ）|
| **Span** | ソースコード内のバイトオフセット範囲 |
| **Symbol Name** | 関数の一意識別子（`ClassName#methodName` 形式）|

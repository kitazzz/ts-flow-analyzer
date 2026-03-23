# ts-flow-analyzer 実装フェーズ解説

このドキュメントは「設計思想」ではなく、「実際のコードがどの順番で何をしているか」を追うための実装ウォークスルーです。

読む順番の基本は次です。

1. [`src/main.rs`](../analyzer/src/main.rs)
2. [`src/ast/collect_functions.rs`](../analyzer/src/ast/collect_functions.rs)
3. 各解析フェーズの入口関数
4. [`src/callgraph/mod.rs`](../analyzer/src/callgraph/mod.rs)
5. [`src/ir/`](../analyzer/src/ir)

---

## 1. 全体像

実行時の大きな流れは次です。

```text
CLI parse
  -> source load
  -> Oxc parse
  -> collect_functions
  -> per-function analyses
       -> metrics
       -> predicates
       -> effects
       -> decision table
       -> data flow
  -> file-level analyses
       -> call graph
       -> Graph IR
  -> JSON / text / DOT output
```

オーケストレーションはほぼすべて [`src/main.rs`](../analyzer/src/main.rs) にあります。各解析器は `main.rs` から呼ばれる独立モジュールです。

---

## 2. Phase 0: CLI と初期化

### 入口

- [`src/main.rs`](../analyzer/src/main.rs)
- [`src/cli/mod.rs`](../analyzer/src/cli/mod.rs)
- [`src/config/mod.rs`](../analyzer/src/config/mod.rs)
- [`src/parser/load_source.rs`](../analyzer/src/parser/load_source.rs)

### 何をしているか

`main()` は最初に次を行います。

1. `Cli::parse()` でフラグを読む
2. `load_config()` で decision table 用設定を読む
3. `load_source()` で対象ファイルを読む
4. `OxcParser::new(...).parse()` で `Program` を得る

### ここで決まる制御フラグ

[`src/main.rs`](../analyzer/src/main.rs) の冒頭で、次の bool が決まります。

- `include_predicates`
- `include_effects`
- `include_decision`
- `include_data_flow`
- `include_call_graph`
- `include_graph`

この設計により、各解析器は「必要なときだけ」起動します。

### feature gate

CFG 系だけは `cfg-analysis` feature が必要です。

- `--cfg-dot`
- `--decision-enhanced`
- `--graph` / `--graph-dot` 内の CFG ノード生成

`cfg-analysis` が無いときでも、それ以外の解析は通常どおり動きます。

---

## 3. Phase 1: 関数・メソッド・クラス収集

### 入口

- `collect_functions()` in [`src/ast/collect_functions.rs`](../analyzer/src/ast/collect_functions.rs)

### 出力

- `Vec<CollectedFunction<'a>>`

### 中核データ

[`src/ast/collect_functions.rs`](../analyzer/src/ast/collect_functions.rs) では、まず AST から解析単位を切り出します。

- `FunctionNode`
  - `Function`
  - `Arrow`
  - `Class`
- `CollectedFunction`
  - `symbol_name`
  - `symbol_kind`
  - `class_name`
  - `member_name`
  - `start_line`
  - `parent_class`
  - `is_abstract`
  - `field_initializers`
  - `has_implicit_super`

### 実装上のポイント

#### 3.1 top-level / export を拾う

`collect_from_statement()` が次を見ます。

- `FunctionDeclaration`
- `VariableDeclaration` の関数代入
- `ClassDeclaration`
- `ExportNamedDeclaration`
- `ExportDefaultDeclaration`

#### 3.2 class は 2 種類の情報を持つ

class 自体も `CollectedFunction` として 1 件作られますが、同時にメソッドも別 `CollectedFunction` として収集されます。

そのため後段では:

- class ノードは構造ノード
- method ノードは解析ノード

として両方使えます。

#### 3.3 constructor 補助情報をここで仕込む

`collect_from_class()` は単にメソッドを集めるだけでなく、後段の effect / call graph 用に:

- field initializer
- 親クラス名
- abstract 情報
- implicit `super()` の有無

を `CollectedFunction` に埋めます。

これは constructor の解析を AST 本体だけでなく class metadata 付きで扱うためです。

---

## 4. Phase 2: per-function 解析ループ

### 入口

- [`src/main.rs`](../analyzer/src/main.rs) の `for func in &collected`

ここが関数ごとの解析をまとめて回す中心です。

```text
for CollectedFunction in collected
  -> get_statements()
  -> compute_metrics()
  -> extract_predicates() -> normalize_predicates()
  -> extract_effects()
  -> build_decision_table()
  -> analyze_data_flow()
  -> FunctionReport に集約
```

### 出力

- `Vec<FunctionReport>`

### `FunctionReport` の役割

[`src/model/mod.rs`](../analyzer/src/model/mod.rs) の `FunctionReport` は、関数単位の統合 DTO です。

- 常に入る: symbol 情報、`metrics`
- フラグ依存: `predicates`, `effects`, `decision_table`, `data_flow`

---

## 5. Phase 2-1: Metrics

### 入口

- `compute_metrics()` in [`src/main.rs`](../analyzer/src/main.rs)

### 呼ばれる関数

- `analyze_basic_complexity()` in [`src/metrics/complexity.rs`](../analyzer/src/metrics/complexity.rs)
- `analyze_max_nesting_depth()` in [`src/metrics/nesting.rs`](../analyzer/src/metrics/nesting.rs)
- `analyze_cyclomatic_complexity()` in [`src/metrics/cyclomatic.rs`](../analyzer/src/metrics/cyclomatic.rs)
- `analyze_function_nesting()` in [`src/metrics/function_nesting.rs`](../analyzer/src/metrics/function_nesting.rs)

### 実装の考え方

metrics は 1 つの巨大 walker ではなく、関心ごとごとに分けています。

- `complexity.rs`
  - `if_count`
  - `else_if_count`
  - `switch_count`
  - `ternary_count`
  - `return_count`
  - `logical_operator_count`
  - `negation_count`
  - `atomic_condition_count`
  - `max_condition_depth`
- `cyclomatic.rs`
  - CC 専用
- `nesting.rs`
  - 制御構造の最大ネスト
- `function_nesting.rs`
  - local function 数
  - function nesting depth
  - callback nesting depth

### 実装上のポイント

- nested function body へ不要に降りないよう、各 walker で skip 条件を持つ
- metrics の合成は `compute_metrics()` で最後に行う

---

## 6. Phase 2-2: Predicate 抽出と正規化

### 入口

- `extract_predicates()` in [`src/predicates/extract.rs`](../analyzer/src/predicates/extract.rs)
- `normalize_predicates()` in [`src/predicates/normalize.rs`](../analyzer/src/predicates/normalize.rs)

### 2 段階構成

#### 6.1 抽出

`extract_predicates()` は statement / expression を walk して `AtomicPredicate` を作ります。

実際の収集は次で行います。

- `extract_from_stmt()`
- `extract_conditionals_from_expr()`
- `collect_from_expr()`

#### 6.2 正規化

`normalize_predicates()` は抽出済み predicate に対して:

- `normalized_name`
- `true_meaning`
- `false_meaning`

を後付けします。

### 実装上のポイント

- 論理式は `collect_from_expr()` で atomic に分解
- `!expr` はテキストを書き換えず `negated` フラグへ吸収
- context は `DecisionContext` として保持
  - `If`
  - `ElseIf`
  - `Ternary`
  - `While`
  - `DoWhile`
  - `For`
  - `Case`

この分離により、predicate の raw 抽出と「LLM / 人が読みやすい名前付け」を別工程にできます。

---

## 7. Phase 2-3: Effect 抽出

### 入口

- `extract_effects()` in [`src/effects/extract.rs`](../analyzer/src/effects/extract.rs)

### 出力

- `Vec<Effect>`

### 何を拾うか

- `return`
- `throw`
- assignment
- call

### 実装上のポイント

#### 7.1 text ベースの side-effect 分類

call の副作用クラスは AST 型ではなくキーワード分類で決めています。

- `Logging`
- `DbWrite`
- `DbRead`
- `ExternalApi`
- `PureCall`

分類関数:

- `classify_call_side_effect()`
- `matches_keyword()`

#### 7.2 constructor 補助 effect

`main.rs` 側では通常の `extract_effects()` に加えて、constructor のときだけ:

- `extract_field_initializer_effects()`
- `extract_parameter_property_effects()`

を併用します。

さらに derived class では:

- implicit `super()` を synthetic effect として先頭に補う
- explicit `super()` がある場合はその直後に initializer effect を差し込む

ここは class 由来の暗黙処理を、人が読める effect 列へ潰している箇所です。

---

## 8. Phase 2-4: Decision Table / MC/DC-like

### 入口

- `build_decision_table()` in [`src/decision/table.rs`](../analyzer/src/decision/table.rs)

### 出力

- `DecisionTableData`
  - `decisions`
  - `truth_rows`
  - `mcdc_cases`
  - `happy_path`

### 中核アルゴリズム

`build_decision_table()` は次の順で進みます。

1. `get_root_statements()` で対象 body を取る
2. `walk_statements()` で path-sensitive に歩く
3. `DecisionPoint` を登録する
4. 終端 path を `TruthRow` に変換する
5. `build_mcdc_cases()` で witness pair を選ぶ

### 実装上のポイント

#### 8.1 decision を重複登録しない

`walk_statements()` は「現在生きている全 path をまとめて」処理します。

そのため同じ `if` 文に複数 path が流れ込んでも、decision index は 1 回だけ増えます。

#### 8.2 try/catch/finally を path として扱う

`walk_statements()` は `try` も専用分岐で処理します。

- try 継続 path
- throw path
- catch 入力 path
- finally 経由 path

を明示的に分けてから再統合します。

#### 8.3 CFG 連携は後付け

`--decision-enhanced` と `cfg-analysis` feature が有効なときだけ、`TruthRow.terminal_reachable` を CFG で補強します。

つまり decision table 本体は AST ベースで常に動き、CFG は reachability の補助として差し込まれます。

---

## 9. Phase 2-5: Local Def-Use / Data Flow

### 入口

- `analyze_data_flow()` in [`src/dataflow/analyze.rs`](../analyzer/src/dataflow/analyze.rs)

### 出力

- `DataFlowReport`
  - `defs`
  - `uses`
  - `def_use_edges`

### 中核構造

`analyze.rs` の中心は `FlowWalker` です。

主な内部状態:

- `ScopeStack`
  - function scope
  - lexical block scope
- `ReachingSet`
  - `binding_id -> Vec<(def_id, may_reach)>`
- `this_field_bindings`
  - `this.foo` ごとの binding
- `def_intern`
  - dry-run と本走査で同じ def site に同じ `DefId` を割り当てる
- `hoisted_fn_spans`
  - body-level hoisted function declaration の識別
- `dry_run`
  - loop 安定化用

### 実装の流れ

`analyze_data_flow()` は概ね次の順で動きます。

1. `seed_params()`
2. `hoist_function_decls()`
3. `hoist_vars()`
4. `walk_stmts()`

### 実装上のポイント

#### 9.1 scope と reaching を分離

名前解決は `ScopeStack`、到達定義は `ReachingSet` で持ちます。

- scope は「どの binding を指すか」
- reaching は「その binding に今どの def が届いているか」

を担当します。

#### 9.2 `var` と lexical scope を分ける

- `declare_var()`
  - function scope に登録
- `declare_lexical()`
  - block scope に登録

この分離で shadowing と `var` hoisting を両立しています。

#### 9.3 loop は dry-run + real-walk

loop body をそのまま複数回 walk すると `Use` や `Edge` が重複します。

そのため実装では:

1. dry-run で reaching set だけ更新
2. merge 後に本走査で use / edge を記録

という 2 段階にしています。

#### 9.4 `mayReach`

分岐 merge は `merge_reaching()` が担当します。

- 全分岐にいる def は `may_reach = false` を維持
- 一部分岐にしかいない def は `may_reach = true`

#### 9.5 `this.field`

`this.field` は通常の lexical binding とは別に、field 名ごとの binding として追跡します。

そのため:

- `this.a`
- `this.b`

は別々の reaching set を持ちます。

---

## 10. Phase 3: File-wide Call Graph

### 入口

- `build_call_graph()` in [`src/callgraph/mod.rs`](../analyzer/src/callgraph/mod.rs)

### 出力

- `CallGraphData`

### 実装の流れ

`build_call_graph()` は次を順に行います。

1. `collect_imports()` で import 一覧を作る
2. `build_class_hierarchy()` で継承関係を作る
3. 各 `CollectedFunction` から `collect_calls()` で `CallSite` を集める
4. `resolve_call()` で内部 / import / 未解決を判定する
5. `classify_call()` で category を付ける
6. `CallEdge` を積む

### 実装上のポイント

#### 10.1 call site 収集と解決を分ける

- `collect_calls.rs`
  - AST から call site を抽出
- `resolve.rs`
  - その call がどこへ向くか解決
- `classify.rs`
  - domain / infra / builtin / unresolved などを分類

という責務分離です。

#### 10.2 継承を考慮した `this.method()`

`build_call_graph()` は `find_concrete_overrides()` を使って、base method から subclass override への dispatch edge も作ります。

#### 10.3 constructor / field initializer call も拾う

通常 body の call だけでなく:

- implicit `super()`
- field initializer 内 call
- parameter property default initializer 内 call

も call graph に入れます。

---

## 11. Phase 4: Graph IR 合成

### 入口

- [`src/ir/builder.rs`](../analyzer/src/ir/builder.rs)
- [`src/ir/graph.rs`](../analyzer/src/ir/graph.rs)

### 生成順

`main.rs` の `--graph` / `--graph-dot` パスでは、`GraphBuilder` に対して段階的にノードと edge を足します。

1. `functions_to_graph()`
2. `callgraph_to_graph()`
3. `decision_to_graph()` 必要時
4. `cfg_to_graph()` 必要時
5. `data_flow_to_graph()` 必要時
6. `builder.build()`

### ノード変換

#### 11.1 functions

- `from_functions.rs`
- `CollectedFunction` -> `Function` / `Class` / `Method`
- class -> method の `Contains` をここで張る

#### 11.2 call graph

- `from_callgraph.rs`
- `CallEdge` ごとに `CallSite` ノードを作る
- internal callee は既存 symbol node に向ける
- import / unresolved は `ExternalSymbol` に落とす

#### 11.3 decision

- `from_decision.rs`
- `DecisionPoint` ノードを作る
- `TruthRow` から `DecisionBranch` edge を導く

#### 11.4 data flow

- `from_data_flow.rs`
- `Def` -> `DataFlowDef`
- `Use` -> `DataFlowUse`
- `DefUseEdge` -> `DataDep`

### 実装上のポイント

Graph IR 自体は非常に薄いです。

- `GraphBuilder::add_node()`
- `GraphBuilder::add_edge()`

しか持たず、各変換器が意味論を付与します。

つまり Graph IR 側に賢いロジックを寄せるのではなく、元データごとの adapter が責務を持つ設計です。

---

## 12. Phase 5: 出力フェーズ

### JSON

`main.rs` では次の条件で top-level の形が変わります。

- `--json` のみ
  - `Vec<FunctionReport>`
- `--call-graph` または `--graph`
  - object
  - `functions`
  - `callGraph`
  - `graph`

### Human-readable

JSON でない場合は `FunctionReport` を text で順に出します。

ここには現在:

- metrics
- predicates
- effects
- data flow

が出ます。

### DOT early exit

次のフラグは専用パスで処理され、通常の JSON / text 出力へ進みません。

- `--cfg-dot`
- `--call-graph-dot`
- `--graph-dot`

この 3 つは `main.rs` 前半で完結します。

---

## 13. 実装を読む順番

最短で全体を掴むなら次の順が読みやすいです。

1. [`src/main.rs`](../analyzer/src/main.rs)
2. [`src/model/mod.rs`](../analyzer/src/model/mod.rs)
3. [`src/ast/collect_functions.rs`](../analyzer/src/ast/collect_functions.rs)
4. [`src/metrics/`](../analyzer/src/metrics)
5. [`src/predicates/extract.rs`](../analyzer/src/predicates/extract.rs)
6. [`src/predicates/normalize.rs`](../analyzer/src/predicates/normalize.rs)
7. [`src/effects/extract.rs`](../analyzer/src/effects/extract.rs)
8. [`src/decision/table.rs`](../analyzer/src/decision/table.rs)
9. [`src/dataflow/analyze.rs`](../analyzer/src/dataflow/analyze.rs)
10. [`src/callgraph/mod.rs`](../analyzer/src/callgraph/mod.rs)
11. [`src/ir/`](../analyzer/src/ir)

特に変更影響を追うときは、まず `main.rs` で「その解析がどの出力パスから呼ばれるか」を確認すると早いです。

---

## 14. 変更時の着眼点

### 新しい関数単位解析を追加したい

見る場所:

- [`src/main.rs`](../analyzer/src/main.rs)
- [`src/model/mod.rs`](../analyzer/src/model/mod.rs)
- 新規 `src/<feature>/`

やること:

1. 解析器を作る
2. `FunctionReport` に optional field を足す
3. `main.rs` の per-function loop へ差し込む
4. README / ARCHITECTURE を更新する

### Graph IR に新しいノード種別を足したい

見る場所:

- [`src/ir/graph.rs`](../analyzer/src/ir/graph.rs)
- [`src/ir/dot.rs`](../analyzer/src/ir/dot.rs)
- `src/ir/from_*.rs`

### call 解決ルールを変えたい

見る場所:

- [`src/callgraph/collect_calls.rs`](../analyzer/src/callgraph/collect_calls.rs)
- [`src/callgraph/resolve.rs`](../analyzer/src/callgraph/resolve.rs)
- [`src/callgraph/classify.rs`](../analyzer/src/callgraph/classify.rs)
- [`src/callgraph/mod.rs`](../analyzer/src/callgraph/mod.rs)

### def-use を拡張したい

見る場所:

- [`src/dataflow/model.rs`](../analyzer/src/dataflow/model.rs)
- [`src/dataflow/analyze.rs`](../analyzer/src/dataflow/analyze.rs)
- [`src/ir/from_data_flow.rs`](../analyzer/src/ir/from_data_flow.rs)

---

## 15. このドキュメントの位置づけ

- 設計意図を知りたい: [`ARCHITECTURE.md`](ARCHITECTURE.md)
- CLI の使い方を知りたい: [`../README.md`](../README.md)
- 実装を読む順番とコード上の責務を知りたい: このドキュメント

実装が変わったら、この文書も `main.rs` の制御フローに合わせて更新してください。

# ts-flow-analyzer

`ts-flow-analyzer` は、TypeScript ソースを対象に関数・メソッド・クラス単位の静的解析を行う Rust 製 CLI です。

- 基本メトリクスの集計
- predicate 抽出
- effect 抽出
- local def-use / data-flow 解析
- decision table / MC/DC-like ケース出力
- intra-file call graph（ファイル内呼び出しグラフ）
- unified Graph IR (`--graph`) と DOT 可視化 (`--graph-dot`)

関連ドキュメント:

- [`REPOSITORY_LAYOUT.md`](REPOSITORY_LAYOUT.md)
- [`ARCHITECTURE.md`](ARCHITECTURE.md)
- [`IMPLEMENTATION_PHASES.md`](IMPLEMENTATION_PHASES.md)
- [`ICFG_SDG_CPG_PLAN.md`](ICFG_SDG_CPG_PLAN.md)

バイナリ名は `ts-flow-analyzer` です。

## インストール

前提:

- Rust toolchain (`cargo`, `rustc`) が使えること
- Rust 1.92.0 以上が必要

確認:

```sh
rustc --version
```

ツールチェイン固定には [`rust-toolchain.toml`](../rust-toolchain.toml) を使っています。repo root から `cargo build` / `cargo install --path analyzer` を実行する前提で、Rust 1.92.0 を要求します。

`rustup` を使っている場合:

```sh
rustup toolchain install 1.92.0
rustup override set 1.92.0
```

`cargo install --path analyzer` で `rustc 1.90.0 is not supported` のように出る場合は、`cargo` が古い toolchain を見ています。まず repo root で次を確認してください。

```sh
rustc --version
cargo --version
```

必要なら toolchain を明示して実行します。

```sh
cargo +1.92.0 install --path analyzer --force
```

推奨は `cargo install --path analyzer` です。これで release ビルド済みの `ts-flow-analyzer` がインストールされます。

```sh
cargo install --path analyzer
```

通常は `~/.cargo/bin/ts-flow-analyzer` に入ります。

### プリビルドバイナリを使う

ソースから build したくない場合は、GitHub Releases のプリビルドバイナリを使えます。

- 対応済み: `macOS arm64`
- 対応済み: `macOS x86_64`
- Releases: <https://github.com/kitazzz/ts-flow-analyzer/releases>

release asset は `ts-flow-analyzer-vX.Y.Z-macos-arm64.tar.gz` または `ts-flow-analyzer-vX.Y.Z-macos-x86_64.tar.gz` という名前で添付されます。

展開例:

```sh
tar -xzf ts-flow-analyzer-v0.1.0-macos-arm64.tar.gz
cd ts-flow-analyzer-v0.1.0-macos-arm64
./ts-flow-analyzer --help
```

`README.md`, `CLI.md`, `config.yaml.example` も同梱しています。

### PATH を通す

`ts-flow-analyzer` をどこからでも呼びたい場合は、`~/.cargo/bin` を PATH に入れます。

`zsh`:

```sh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

確認:

```sh
which ts-flow-analyzer
ts-flow-analyzer --help
```

更新時は再インストールします。

```sh
cargo install --path analyzer --force
```

CFG 拡張を使う場合は feature 付きでインストールします。

```sh
cargo install --path analyzer --features cfg-analysis --force
```

## GitHub Releases 配布

`v*` tag を push すると、GitHub Actions が次の 2 種類の macOS バイナリを自動で build して release asset に添付します。

- `macos-15` runner: `aarch64-apple-darwin` (`macOS arm64`)
- `macos-15-intel` runner: `x86_64-apple-darwin` (`macOS x86_64`)

workflow ファイルは [`release-macos.yml`](../.github/workflows/release-macos.yml) です。tag を切らずに Actions から `workflow_dispatch` で流した場合も、artifact として同じ tarball を取得できます。

コマンドは 1 行で実行してください。途中で改行すると `-force` のように解釈されて失敗します。

## クイックスタート

最初に覚えるのはこの 4 パターンで十分です。

最小実行:

```sh
ts-flow-analyzer /absolute/path/to/file.ts
```

JSON で一通り見る:

```sh
ts-flow-analyzer /absolute/path/to/file.ts --all --json
```

Graph IR まで見る:

```sh
ts-flow-analyzer /absolute/path/to/file.ts --graph --decision --data-flow
```

特定メソッドだけを見る:

```sh
ts-flow-analyzer /absolute/path/to/file.ts --function 'ClassName#methodName'
```

このリポジトリのサンプルを解析する場合:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --all --json
```

## 使い方

ヘルプ:

```sh
ts-flow-analyzer --help
```

CLI 仕様:

```sh
ts-flow-analyzer [OPTIONS] <FILE>
```

`<FILE>` は解析対象のソースファイルです。実装上はファイル拡張子から `SourceType` を判定しています。現状のサンプルと主用途は `.ts` です。

`config.yaml` がカレントディレクトリにあれば自動で読み込みます。別パスを使う場合は `--config <PATH>` を指定します。

### オプション

最初は `--all --json` から始めるのが一番わかりやすいです。`--graph` は別系統の大きい出力なので、必要になってから足す方が見やすいです。

入力:

- `--config <PATH>`: analyzer 設定 YAML を明示指定する

出力と絞り込み:

- `--json`: JSON で出力する
- `--function <FUNCTION>`: 特定シンボルだけに絞る

解析レイヤ:

- `--all`: `predicates` / `effects` / `data-flow` / `decision` / `call-graph` をまとめて有効化する
- `--predicates`: predicate を含める
- `--effects`: effect を含める
- `--data-flow`: local def-use / data-flow を含める
- `--decision`: decision table を含める
- `--call-graph`: call graph を JSON 出力に含める
- `--graph`: unified Graph IR を JSON 出力に含める（`--json` を暗黙に有効化）

DOT 可視化:

- `--call-graph-dot <FUNCTION>`: 指定した function からの到達可能な呼び出しグラフを DOT で stdout に出す
- `--graph-dot <FUNCTION>`: 指定した function / method を起点に Graph IR を DOT で stdout に出す
- `--cfg-dot <FUNCTION>`: 指定した function / method の CFG を DOT で stdout に出す

高度なオプション:

- `--decision-enhanced`: decision table を CFG 拡張モードで出す
- `--include-builtin-calls`: builtin/collection メソッド（`reduce`, `some`, `toLocaleString` 等）を call graph に含める（デフォルトでは非表示）

補足:

- metrics は常に出力されます
- `--function` は `symbolName` を指定します
- class method の `symbolName` は `ClassName#methodName` 形式です
- `--decision-enhanced` と `--cfg-dot` で CFG 機能を使うには `cfg-analysis` feature 付きビルドが必要です
- `--graph` / `--graph-dot` は feature なしでも使えますが、`cfg-analysis` 付きビルドのときだけ `cfgBlock` / `cfg` edge が含まれます
- `--graph` は常にファイル全体を graph 化します。`--function` は `functions` 配列のみに適用され、`graph` キーは絞り込みません

## 実行例

基本メトリクスを見る:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts
```

`ApproveOrderUseCase#execute` だけを見る:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --function 'ApproveOrderUseCase#execute'
```

predicate と effect を JSON で見る:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --predicates --effects --json
```

data flow を JSON で見る:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --data-flow --json
```

decision table まで含めて出す:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --all --json
```

CFG 拡張付き decision table を JSON で出す:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --decision --decision-enhanced --json
```

config を明示して decision table を出す:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --decision --json --config analyzer/config.yaml.example
```

call graph を JSON で見る:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --call-graph --json
```

Graph IR を JSON で見る:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --graph
```

Graph IR と decision point をまとめて JSON で見る:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --graph --decision
```

Graph IR に data flow も含めて見る:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --graph --data-flow
```

call graph を DOT で出す（execute からの到達可能グラフ）:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --call-graph-dot 'ApproveOrderUseCase#execute'
```

特定メソッドの CFG を DOT で出す:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#execute'
```

DOT をファイルに保存する:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#execute' > execute.dot
```

Graph IR を DOT で出す:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --graph-dot 'ApproveOrderUseCase#execute'
```

別サンプルを解析する:

```sh
ts-flow-analyzer samples/domain/order.ts
ts-flow-analyzer samples/utils/validation.ts --all --json
```

## 何が出るか

レポート対象:

- named function declaration
- 変数に代入された function / arrow function
- class declaration
- class method

JSON では各シンボルごとに `FunctionReport` 相当のオブジェクトを返します。

主なフィールド:

- `symbolName`: 表示用の識別子
- `symbolKind`: `function` / `variableFunction` / `method` / `class`
- `className`: 所属 class 名
- `memberName`: method や function の元名
- `functionName`: 後方互換用の名前
- `filePath`: 入力ファイルパス
- `startLine`: シンボル開始行
- `metrics`: 複雑性メトリクス
- `predicates`: 条件式の抽出結果
- `effects`: `return` / `throw` / call / assignment などの抽出結果
- `dataFlow`: local intraprocedural def-use / data-flow 結果
- `decisionTable`: decision table / MC/DC-like ケース

`callGraph` と `graph` は `FunctionReport` の中ではなく、トップレベルのオブジェクト形式で返ります。

### JSON 出力形式の注意

`--call-graph`、`--all`、または `--graph` を指定すると、JSON のトップレベルが配列からオブジェクトに変わります。

```json
{
  "functions": [ /* FunctionReport の配列。--data-flow 時は dataFlow を含む */ ],
  "callGraph": { "nodes": [...], "edges": [...], "imports": [...] },
  "graph": { "filePath": "...", "nodes": [...], "edges": [...] }
}
```

`callGraph` は `--call-graph` / `--all` のときだけ、`graph` は `--graph` のときだけ含まれます。これらのフラグなしでは従来通り `[FunctionReport]` の配列を返します。

### metrics

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
- `logicalOperatorCount`
- `negationCount`
- `atomicConditionCount`
- `maxConditionDepth`

`atomicConditionCount` は条件式（`if` / `while` / `for` / 三項演算子）内の末端条件の数です。`a && b` は 2 つの atomic condition です。`maxConditionDepth` は `&&` / `||` のネスト深さの最大値です。

### predicates

`--predicates` または `--all` を付けると、条件式ごとに次のような情報を返します。

- `text`
- `negated`
- `kind`
- `context`
- `line`
- `normalizedName`
- `trueMeaning`
- `falseMeaning`

`normalizedName` は元式の意味に応じた 3 カテゴリで命名されます。

| カテゴリ | 例 | normalizedName |
|---------|---|---------------|
| nullable/object | `!order` | `orderMissing` |
| boolean property | `!riskCheck.ok`, `!input.forceApprove` | `riskCheckNotOk`, `inputForceApproveDisabled` |
| already-meaningful | `hasZeroPriceItem` | `hasZeroPriceItem`（そのまま） |

### effects

`--effects` または `--all` を付けると、次のような effect を返します。

- `kind`: `return` / `throw` / `assignment` / `call`
- `sideEffect`: `dbRead` / `dbWrite` / `externalApi` / `logging` / `stateWrite` / `pureCall` など
- `text`
- `line`

### data flow

`--data-flow` または `--all` を付けると、関数ごとに `dataFlow` を返します。

- `defs`: local binding / assignment / for-binding / catch-binding の定義点
- `uses`: identifier read / `this.field` read の使用点
- `defUseEdges`: 到達した定義から使用への edge

`defUseEdges[].mayReach` は分岐マージ由来の「一部分岐でのみ到達する」edge を表します。

- `false`: must-reach。再定義されずに到達
- `true`: may-reach。分岐の一部でのみ到達

初期化なし宣言（`let x: number;`）は `defs` には入りますが、reaching def には入らないため `defUseEdges` の source にはなりません。

### decision table

`--decision` または `--all` を付けると、次を返します。

- `decisions`: predicate 一覧
- `truthRows`: 経路ごとの真理値と outcome
- `mcdcCases`: predicate ごとの witness pair
- `happyPath`: 成功経路

### config.yaml

decision table の `happyPath` 判定と `Success` / `Failure` ラベルは `config.yaml` で調整できます。

例:

```yaml
decision_table:
  success_when_true:
    - ok
    - allowed
    - success
  failure_when_false:
    - ok
    - allowed
    - success
  failure_when_present:
    - error
    - reason
```

意味:

- `success_when_true`: `return { ok: true }` のように、`true` なら成功とみなすキー
- `failure_when_false`: `return { ok: false }` のように、`false` なら失敗とみなすキー
- `failure_when_present`: `return { error: '...' }` のように、キーが存在したら失敗とみなすキー

未指定時は上のデフォルトが使われます。サンプルは [`analyzer/config.yaml.example`](../analyzer/config.yaml.example) にあります。

この出力は厳密な形式検証としての strict MC/DC ではなく、現状は branch-sensitive な近似出力です。

`--decision-enhanced` を付けると次の拡張が有効になります。

- `&&` / `||` を short-circuit aware な複数列に展開する
- `truthRows[].terminalReachable` を追加する

`terminalReachable` は truth row 全体の feasibility ではなく、終端 `return` / `throw` が属する CFG block が dead code 扱いかどうかを表します。

### call graph

`--call-graph` または `--all` を付けると、ファイル内の関数間呼び出し関係を返します。

- `nodes`: ファイル内の全関数/メソッド（class ノードを除く）
- `edges`: 各関数から呼ばれる call site ごとに 1 エッジ
- `imports`: ファイル先頭の `import` 宣言（type-only は除外）

各 edge の `kind`:

| kind | 例 | 解決 |
|------|---|------|
| `direct` | `canTransition()` | 同一ファイル内関数 or import に解決 |
| `thisMethod` | `this.checkRisk()` | 同一クラスまたは親クラスのメソッドに解決 |
| `memberCall` | `this.orderRepo.findById()` | 未解決（receiver chain を記録） |
| `super` | `super.method()` | 親クラスチェーン上のメソッドに解決 |
| `superConstructor` | `super()` | 親クラスの constructor に解決 |
| `new` | `new Foo()` | 同一ファイル class / function または import に解決 |

`callee` が `null` の edge は未解決です。`importSource` がある場合はインポート経由の外部呼び出しです。

各 edge には `callCategory` フィールドが付きます。

| callCategory | 説明 | 例 |
|-------------|------|---|
| `domain` | 解決済みの内部関数 / import | `checkRisk`, `canTransition` |
| `infra` | インフラ層の呼び出し（repo, client 等） | `this.orderRepo.findById` |
| `builtin` | 組み込み / collection メソッド | `reduce`, `some`, `toLocaleString` |
| `unresolved` | 解決不能 | dynamic call |

デフォルトでは `builtin` は非表示です。`--include-builtin-calls` で表示できます。

### graph

`--graph` は file-scope の unified Graph IR を返します。ノードとエッジの基本形は次です。

- `nodes[].kind`: `function` / `class` / `method` / `callSite` / `externalSymbol` / `decisionPoint` / `cfgBlock` / `dataFlowDef` / `dataFlowUse`
- `nodes[].loc`: `line`, `spanStart`, `spanEnd`
- `edges[].kind.type`: `contains` / `call` / `decisionBranch` / `cfg` / `dataDep`

現在の構成は次のとおりです。

- function / class / method ノード
- caller `-[contains]->` callsite
- callsite `-[call]->` internal callee または external symbol
- function `-[contains]->` decision point
- decision point `-[decisionBranch]->` 次の decision point（truth row 由来の要約 edge）
- function `-[contains]->` dataFlowDef / dataFlowUse（`--data-flow` または `--all` 時）
- dataFlowDef `-[dataDep]->` dataFlowUse
- function `-[contains]->` cfgBlock（`cfg-analysis` 付きビルド時）
- cfgBlock `-[cfg]->` cfgBlock

補足:

- `CallSite` は call expression の span を持ちます
- `ExternalSymbol` はローカルソース上の span を持ちません
- `DataFlowDef` / `DataFlowUse` は source location を保持します
- decision graph はまだ outcome node まで含む完全 DAG ではありません。現状の `decisionBranch` は truth row から導いた summary edge です
- `--graph` 単体では decision point は出ません。`--decision` または `--all` を併用したときだけ含まれます
- `--graph` 単体では data-flow ノードは出ません。`--data-flow` または `--all` を併用したときだけ含まれます

例:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --graph --decision
```

### call graph DOT

`--call-graph-dot <FUNCTION>` は、指定エントリポイントから到達可能な呼び出しグラフを DOT で出力して終了します。

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --call-graph-dot 'ApproveOrderUseCase#execute'
```

ノードの色分け:

- 黄色（実線）: ファイル内の関数/メソッド
- 水色（破線）: インポート経由の外部関数
- 赤色（点線）: 未解決の呼び出し先

SVG 化:

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --call-graph-dot 'ApproveOrderUseCase#execute' > callgraph.dot
dot -Tsvg callgraph.dot -o callgraph.svg
```

### graph DOT

`--graph-dot <FUNCTION>` は、指定した function / method を root にして Graph IR の部分グラフを DOT で出力します。

- root function / method
- その `contains` 子孫
- そこから出る `call` / `cfg` / `decisionBranch` の 1-hop target

`--decision` を付けると decision point と `decisionBranch` も含まれます。`--data-flow` を付けると `dataFlowDef` / `dataFlowUse` と橙色の `dataDep` edge も含まれます。`cfg-analysis` 付きビルドなら `cfgBlock` も含まれます。

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --graph-dot 'ApproveOrderUseCase#execute'
ts-flow-analyzer samples/usecase/approveOrder.ts --graph-dot 'ApproveOrderUseCase#execute' --decision
ts-flow-analyzer samples/usecase/approveOrder.ts --graph-dot 'ApproveOrderUseCase#execute' --data-flow
```

### CFG / DOT

`--cfg-dot <FUNCTION>` は、指定した function / method の CFG 部分グラフを DOT 形式で stdout に出して終了します。

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#checkRisk'
```

Graphviz が入っていれば SVG 化できます。

```sh
ts-flow-analyzer samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#execute' > execute.dot
dot -Tsvg execute.dot -o execute.svg
```

## 出力イメージ

`ts-flow-analyzer samples/usecase/approveOrder.ts --function 'ApproveOrderUseCase#execute' --graph --decision --json`

```json
{
  "functions": [
    {
      "symbolName": "ApproveOrderUseCase#execute",
      "symbolKind": "method",
      "className": "ApproveOrderUseCase",
      "memberName": "execute",
      "functionName": "execute",
      "filePath": "samples/usecase/approveOrder.ts",
      "startLine": 26
    }
  ],
  "graph": {
    "filePath": "samples/usecase/approveOrder.ts",
    "nodes": [
      {
        "id": 2,
        "kind": "method",
        "label": "ApproveOrderUseCase#execute",
        "symbolName": "ApproveOrderUseCase#execute",
        "loc": { "line": 26, "spanStart": 728, "spanEnd": 1726 }
      },
      {
        "id": 4,
        "kind": "callSite",
        "label": "findById",
        "loc": { "line": 27, "spanStart": 810, "spanEnd": 848 }
      },
      {
        "id": 15,
        "kind": "decisionPoint",
        "label": "!order",
        "loc": { "line": 28, "spanStart": 857, "spanEnd": 863 }
      }
    ],
    "edges": [
      { "source": 2, "target": 4, "kind": { "type": "contains" } },
      { "source": 4, "target": 5, "kind": { "type": "call", "callKind": "memberCall", "category": "infra" } },
      { "source": 15, "target": 16, "kind": { "type": "decisionBranch", "branch": false } }
    ]
  }
}
```

**注意**: `--all` や `--call-graph` や `--graph` を付けない場合は、従来通り `[FunctionReport]` の配列が返ります。

## 開発メモ

- 日常利用は `cargo install --path analyzer` で入れた `ts-flow-analyzer` を使う前提です
- 開発中に未インストールの状態で試すなら `cargo run --manifest-path analyzer/Cargo.toml -- ...` でも実行できます
- class 自体もレポート対象ですが、class ノードには statement body がないため、class の metrics は最小値寄りになります
- サンプルはリポジトリルートの `samples/` にあります

## よく使うコマンド

```sh
# インストール
cargo install --path analyzer

# 更新
cargo install --path analyzer --force

# CFG 拡張付きで更新
cargo install --path analyzer --features cfg-analysis --force

# ヘルプ
ts-flow-analyzer --help

# 解析
ts-flow-analyzer samples/usecase/placeOrder.ts

# JSON 出力
ts-flow-analyzer samples/usecase/placeOrder.ts --json

# 詳細全部
ts-flow-analyzer samples/usecase/placeOrder.ts --all --json

# data-flow のみ JSON
ts-flow-analyzer samples/usecase/placeOrder.ts --data-flow --json

# CFG 拡張付き decision table
ts-flow-analyzer samples/usecase/approveOrder.ts --decision --decision-enhanced --json

# call graph 付き JSON
ts-flow-analyzer samples/usecase/approveOrder.ts --call-graph --json

# Graph IR
ts-flow-analyzer samples/usecase/approveOrder.ts --graph

# Graph IR + decision points
ts-flow-analyzer samples/usecase/approveOrder.ts --graph --decision

# Graph IR + data-flow
ts-flow-analyzer samples/usecase/approveOrder.ts --graph --data-flow

# call graph DOT（エントリポイント指定）
ts-flow-analyzer samples/usecase/approveOrder.ts --call-graph-dot 'ApproveOrderUseCase#execute'

# Graph IR DOT
ts-flow-analyzer samples/usecase/approveOrder.ts --graph-dot 'ApproveOrderUseCase#execute'

# 関数 CFG を DOT 出力
ts-flow-analyzer samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#execute'

# 特定メソッドだけ
ts-flow-analyzer samples/usecase/approveOrder.ts --function 'ApproveOrderUseCase#checkRisk' --all --json
```

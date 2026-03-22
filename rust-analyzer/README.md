# recast-forge-analyzer

`recast-forge-analyzer` は、TypeScript ソースを対象に関数・メソッド・クラス単位の静的解析を行う Rust 製 CLI です。

- 基本メトリクスの集計
- predicate 抽出
- effect 抽出
- decision table / MC/DC-like ケース出力
- intra-file call graph（ファイル内呼び出しグラフ）

バイナリ名は `rf-analyze` です。

## インストール

前提:

- Rust toolchain (`cargo`, `rustc`) が使えること
- Rust 1.92.0 以上が必要

確認:

```sh
rustc --version
```

このディレクトリには [rust-toolchain.toml](/Users/kitazzz/.superset/projects/recast-forge/rust-analyzer/rust-toolchain.toml) を置いてあり、`rustup` 管理の環境なら 1.92.0 を使う前提です。

`rustup` を使っている場合:

```sh
rustup toolchain install 1.92.0
rustup override set 1.92.0
```

推奨は `cargo install --path .` です。これで release ビルド済みの `rf-analyze` がインストールされます。

```sh
cd rust-analyzer
cargo install --path .
```

通常は `~/.cargo/bin/rf-analyze` に入ります。

### PATH を通す

`rf-analyze` をどこからでも呼びたい場合は、`~/.cargo/bin` を PATH に入れます。

`zsh`:

```sh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

確認:

```sh
which rf-analyze
rf-analyze --help
```

更新時は再インストールします。

```sh
cd rust-analyzer
cargo install --path . --force
```

CFG 拡張を使う場合は feature 付きでインストールします。

```sh
cd rust-analyzer
cargo install --path . --features cfg-analysis --force
```

コマンドは 1 行で実行してください。途中で改行すると `-force` のように解釈されて失敗します。

## クイックスタート

最小実行:

```sh
rf-analyze /absolute/path/to/file.ts
```

JSON で詳細出力:

```sh
rf-analyze /absolute/path/to/file.ts --all --json
```

このリポジトリのサンプルを解析する場合:

```sh
cd rust-analyzer
rf-analyze ../samples/usecase/approveOrder.ts --all --json
```

## 使い方

ヘルプ:

```sh
rf-analyze --help
```

CLI 仕様:

```sh
rf-analyze [OPTIONS] <FILE>
```

`<FILE>` は解析対象のソースファイルです。実装上はファイル拡張子から `SourceType` を判定しています。現状のサンプルと主用途は `.ts` です。

### オプション

- `--json`: JSON で出力する
- `--predicates`: predicate を含める
- `--effects`: effect を含める
- `--decision`: decision table を含める
- `--decision-enhanced`: decision table を CFG 拡張モードで出す
- `--all`: `predicates` / `effects` / `decision` / `call-graph` をまとめて有効化する
- `--function <FUNCTION>`: 特定シンボルだけに絞る
- `--call-graph`: call graph を JSON 出力に含める
- `--call-graph-dot <FUNCTION>`: 指定した function からの到達可能な呼び出しグラフを DOT で stdout に出す
- `--include-builtin-calls`: builtin/collection メソッド（`reduce`, `some`, `toLocaleString` 等）を call graph に含める（デフォルトでは非表示）
- `--cfg-dot <FUNCTION>`: 指定した function / method の CFG を DOT で stdout に出す

補足:

- metrics は常に出力されます
- `--function` は `symbolName` を指定します
- class method の `symbolName` は `ClassName#methodName` 形式です
- `--decision-enhanced` と `--cfg-dot` で CFG 機能を使うには `cfg-analysis` feature 付きビルドが必要です

## 実行例

基本メトリクスを見る:

```sh
rf-analyze ../samples/usecase/approveOrder.ts
```

`ApproveOrderUseCase#execute` だけを見る:

```sh
rf-analyze ../samples/usecase/approveOrder.ts --function 'ApproveOrderUseCase#execute'
```

predicate と effect を JSON で見る:

```sh
rf-analyze ../samples/usecase/approveOrder.ts --predicates --effects --json
```

decision table まで含めて出す:

```sh
rf-analyze ../samples/usecase/approveOrder.ts --all --json
```

CFG 拡張付き decision table を JSON で出す:

```sh
rf-analyze ../samples/usecase/approveOrder.ts --decision --decision-enhanced --json
```

call graph を JSON で見る:

```sh
rf-analyze ../samples/usecase/approveOrder.ts --call-graph --json
```

call graph を DOT で出す（execute からの到達可能グラフ）:

```sh
rf-analyze ../samples/usecase/approveOrder.ts --call-graph-dot 'ApproveOrderUseCase#execute'
```

特定メソッドの CFG を DOT で出す:

```sh
rf-analyze ../samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#execute'
```

DOT をファイルに保存する:

```sh
rf-analyze ../samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#execute' > execute.dot
```

別サンプルを解析する:

```sh
rf-analyze ../samples/domain/order.ts
rf-analyze ../samples/utils/validation.ts --all --json
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
- `decisionTable`: decision table / MC/DC-like ケース
- `callGraph`: ファイル内呼び出しグラフ（`--call-graph` または `--all` 時のみ、トップレベル構造が変わる）

### JSON 出力形式の注意

`--call-graph` または `--all` を指定すると、JSON のトップレベルが配列からオブジェクトに変わります。

```json
{
  "functions": [ /* FunctionReport の配列 */ ],
  "callGraph": { "nodes": [...], "edges": [...], "imports": [...] }
}
```

これらのフラグなしでは従来通り `[FunctionReport]` の配列を返します。

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

### decision table

`--decision` または `--all` を付けると、次を返します。

- `decisions`: predicate 一覧
- `truthRows`: 経路ごとの真理値と outcome
- `mcdcCases`: predicate ごとの witness pair
- `happyPath`: 成功経路

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
| `thisMethod` | `this.checkRisk()` | 同一クラス内メソッドに解決 |
| `memberCall` | `this.orderRepo.findById()` | 未解決（receiver chain を記録） |
| `super` | `super.method()` | 未解決（親クラス解決は将来対応） |

`callee` が `null` の edge は未解決です。`importSource` がある場合はインポート経由の外部呼び出しです。

各 edge には `callCategory` フィールドが付きます。

| callCategory | 説明 | 例 |
|-------------|------|---|
| `domain` | 解決済みの内部関数 / import | `checkRisk`, `canTransition` |
| `infra` | インフラ層の呼び出し（repo, client 等） | `this.orderRepo.findById` |
| `builtin` | 組み込み / collection メソッド | `reduce`, `some`, `toLocaleString` |
| `unresolved` | 解決不能 | dynamic call |

デフォルトでは `builtin` は非表示です。`--include-builtin-calls` で表示できます。

### call graph DOT

`--call-graph-dot <FUNCTION>` は、指定エントリポイントから到達可能な呼び出しグラフを DOT で出力して終了します。

```sh
rf-analyze ../samples/usecase/approveOrder.ts --call-graph-dot 'ApproveOrderUseCase#execute'
```

ノードの色分け:

- 黄色（実線）: ファイル内の関数/メソッド
- 水色（破線）: インポート経由の外部関数
- 赤色（点線）: 未解決の呼び出し先

SVG 化:

```sh
rf-analyze ../samples/usecase/approveOrder.ts --call-graph-dot 'ApproveOrderUseCase#execute' > callgraph.dot
dot -Tsvg callgraph.dot -o callgraph.svg
```

### CFG / DOT

`--cfg-dot <FUNCTION>` は、指定した function / method の CFG 部分グラフを DOT 形式で stdout に出して終了します。

```sh
rf-analyze ../samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#checkRisk'
```

Graphviz が入っていれば SVG 化できます。

```sh
rf-analyze ../samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#execute' > execute.dot
dot -Tsvg execute.dot -o execute.svg
```

## 出力イメージ

`rf-analyze ../samples/usecase/approveOrder.ts --function 'ApproveOrderUseCase#execute' --all --json`

```json
{
  "functions": [
    {
      "symbolName": "ApproveOrderUseCase#execute",
      "symbolKind": "method",
      "className": "ApproveOrderUseCase",
      "memberName": "execute",
      "functionName": "execute",
      "filePath": "../samples/usecase/approveOrder.ts",
      "startLine": 26,
      "metrics": {
        "ifCount": 6,
        "cyclomaticComplexity": 7,
        "maxNestingDepth": 2
      },
      "predicates": [
        {
          "text": "order",
          "negated": true,
          "kind": "truthiness",
          "context": "if",
          "line": 28,
          "normalizedName": "orderMissing"
        }
      ],
      "effects": [
        {
          "kind": "call",
          "sideEffect": "dbRead",
          "line": 27,
          "text": "const order = await this.orderRepo.findById(input.orderId)"
        }
      ],
      "decisionTable": {
        "symbolName": "ApproveOrderUseCase#execute"
      }
    }
  ],
  "callGraph": {
    "nodes": [
      { "symbolName": "ApproveOrderUseCase#execute", "kind": "method", "className": "ApproveOrderUseCase", "startLine": 26 },
      { "symbolName": "ApproveOrderUseCase#checkRisk", "kind": "method", "className": "ApproveOrderUseCase", "startLine": 56 }
    ],
    "edges": [
      { "caller": "ApproveOrderUseCase#execute", "callee": "ApproveOrderUseCase#checkRisk", "kind": "thisMethod", "targetName": "checkRisk", "receiver": "this", "line": 45 },
      { "caller": "ApproveOrderUseCase#execute", "kind": "memberCall", "targetName": "findById", "receiver": "this.orderRepo", "line": 27 },
      { "caller": "ApproveOrderUseCase#execute", "kind": "direct", "targetName": "canTransition", "line": 40, "importSource": "../domain/order.ts" }
    ],
    "imports": [
      { "localName": "canTransition", "importedName": "canTransition", "source": "../domain/order.ts" },
      { "localName": "transitionOrder", "importedName": "transitionOrder", "source": "../domain/order.ts" }
    ]
  }
}
```

**注意**: `--all` や `--call-graph` を付けない場合は、従来通り `[FunctionReport]` の配列が返ります。

## 開発メモ

- 日常利用は `cargo install --path .` で入れた `rf-analyze` を使う前提です
- 開発中に未インストールの状態で試すなら `cargo run -- ...` でも実行できます
- class 自体もレポート対象ですが、class ノードには statement body がないため、class の metrics は最小値寄りになります
- サンプルはリポジトリルートの `samples/` にあります

## よく使うコマンド

```sh
# インストール
cargo install --path .

# 更新
cargo install --path . --force

# CFG 拡張付きで更新
cargo install --path . --features cfg-analysis --force

# ヘルプ
rf-analyze --help

# 解析
rf-analyze ../samples/usecase/placeOrder.ts

# JSON 出力
rf-analyze ../samples/usecase/placeOrder.ts --json

# 詳細全部
rf-analyze ../samples/usecase/placeOrder.ts --all --json

# CFG 拡張付き decision table
rf-analyze ../samples/usecase/approveOrder.ts --decision --decision-enhanced --json

# call graph 付き JSON
rf-analyze ../samples/usecase/approveOrder.ts --call-graph --json

# call graph DOT（エントリポイント指定）
rf-analyze ../samples/usecase/approveOrder.ts --call-graph-dot 'ApproveOrderUseCase#execute'

# 関数 CFG を DOT 出力
rf-analyze ../samples/usecase/approveOrder.ts --cfg-dot 'ApproveOrderUseCase#execute'

# 特定メソッドだけ
rf-analyze ../samples/usecase/approveOrder.ts --function 'ApproveOrderUseCase#checkRisk' --all --json
```

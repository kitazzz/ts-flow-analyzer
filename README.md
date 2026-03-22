# recast-forge

TypeScript コードベースに対する静的解析の実験リポジトリです。

現在の主実装は Rust / Oxc ベースの CLI `rust-analyzer/` にあります。`src/` 配下の TypeScript CLI は初期 PoC / 探索用の実装として残しています。

## Current Entry Points

- 現行 CLI と機能一覧: [`rust-analyzer/README.md`](rust-analyzer/README.md)
- 実装構成とデータモデル: [`rust-analyzer/docs/ARCHITECTURE.md`](rust-analyzer/docs/ARCHITECTURE.md)
- フェーズ別の実装解説: [`rust-analyzer/docs/IMPLEMENTATION_PHASES.md`](rust-analyzer/docs/IMPLEMENTATION_PHASES.md)
- マイルストーンと方針: [`plan/milestone.md`](plan/milestone.md)
- legacy Node CLI ガイド: [`docs/CLI_GUIDE.md`](docs/CLI_GUIDE.md)

## Quick Start

```sh
# 基本ビルド
cargo build --manifest-path rust-analyzer/Cargo.toml

# JSON で主要解析をまとめて出す
cargo run --manifest-path rust-analyzer/Cargo.toml -- \
  samples/usecase/approveOrder.ts --all --json

# local def-use / data-flow を出す
cargo run --manifest-path rust-analyzer/Cargo.toml -- \
  samples/usecase/approveOrder.ts --data-flow --json

# Graph IR に data-flow も含めて出す
cargo run --manifest-path rust-analyzer/Cargo.toml -- \
  samples/usecase/approveOrder.ts --graph --data-flow
```

現行 CLI で扱える主な出力:

- metrics
- predicates
- effects
- local def-use / data-flow
- decision table / MC/DC-like cases
- intra-file call graph
- unified Graph IR / DOT

## Legacy Prototype

以下は `src/cli/analyze.ts` を中心とした TypeScript PoC の説明です。現行の主系 CLI ではないため、新しい機能や最新仕様は `rust-analyzer/README.md` を優先してください。

## Usage

```sh
npx tsx src/cli/analyze.ts <file.ts> [options]

Options:
  --json                  Output as raw JSON
  --top <n>               Show top N symbols by cyclomaticComplexity
  --min-complexity <n>    Show symbols with cyclomaticComplexity >= N
```

## Examples

### オプション基本

```sh
# デフォルト (pretty print、complexity 降順)
npx tsx src/cli/analyze.ts samples/usecase/placeOrder.ts

# JSON 出力
npx tsx src/cli/analyze.ts samples/usecase/placeOrder.ts --json

# 上位3件のみ
npx tsx src/cli/analyze.ts samples/domain/order.ts --top 3

# CC >= 5 の関数のみ
npx tsx src/cli/analyze.ts samples/utils/validation.ts --min-complexity 5

# 組み合わせ
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --min-complexity 4 --json
```

### サンプルファイル一覧

```sh
# utils: 文字列操作 (CC 1〜4、シンプルな関数が多い)
npx tsx src/cli/analyze.ts samples/utils/string.ts

# utils: バリデーション (CC 5〜7、条件分岐が多い)
npx tsx src/cli/analyze.ts samples/utils/validation.ts

# domain: ユーザーモデル (CC 1〜4、switch / ternary を含む)
npx tsx src/cli/analyze.ts samples/domain/user.ts

# domain: 注文モデル (CC 1〜6、coupon 処理が複雑)
npx tsx src/cli/analyze.ts samples/domain/order.ts

# repository: インメモリ実装 (class method を解析、search が CC=7)
npx tsx src/cli/analyze.ts samples/repository/orderRepository.ts
npx tsx src/cli/analyze.ts samples/repository/userRepository.ts

# usecase: 注文確定 (PlaceOrderUseCase#execute が CC=8)
npx tsx src/cli/analyze.ts samples/usecase/placeOrder.ts

# usecase: 注文承認 (execute CC=7、checkRisk CC=4)
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts

# usecase: 注文キャンセル (execute CC=6)
npx tsx src/cli/analyze.ts samples/usecase/cancelOrder.ts
```

### まとめて複雑な関数を探す (shell ループ)

```sh
for f in \
  samples/utils/string.ts \
  samples/utils/validation.ts \
  samples/domain/user.ts \
  samples/domain/order.ts \
  samples/repository/orderRepository.ts \
  samples/repository/userRepository.ts \
  samples/usecase/placeOrder.ts \
  samples/usecase/approveOrder.ts \
  samples/usecase/cancelOrder.ts; do
  echo "=== $f ===" && npx tsx src/cli/analyze.ts "$f" --min-complexity 5
done
```

## Sample Output

```json
[
  {
    "symbolName": "ApproveOrderUseCase",
    "symbolKind": "class",
    "className": "ApproveOrderUseCase",
    "functionName": "ApproveOrderUseCase",
    "filePath": "/.../samples/usecase/approveOrder.ts",
    "startLine": 20,
    "metrics": {
      "ifCount": 9,
      "elseIfCount": 0,
      "switchCount": 0,
      "ternaryCount": 0,
      "returnCount": 10,
      "maxNestingDepth": 2,
      "cyclomaticComplexity": 10,
      "localFunctionCount": 4,
      "functionNestingDepth": 2,
      "callbackNestingDepth": 1,
      "logicalOperatorCount": 1
    }
  }
]
```

## Metrics

| Metric | Description |
|---|---|
| `ifCount` | `if` statement count (includes else-if) |
| `elseIfCount` | `else if` only |
| `switchCount` | `switch` statement count |
| `ternaryCount` | Conditional expression (`? :`) count |
| `returnCount` | `return` statement count |
| `maxNestingDepth` | Max control flow nesting (if/switch/for/while/try) |
| `cyclomaticComplexity` | 1 + if + caseClause + ternary |
| `localFunctionCount` | All nested functions inside body |
| `functionNestingDepth` | Max function nesting depth (root = 0) |
| `callbackNestingDepth` | Max depth of callbacks in call/new expression args |
| `logicalOperatorCount` | `&&` / `\|\|` count |

## Report Targets

- `FunctionDeclaration` (named functions)
- `VariableDeclaration` whose initializer is `ArrowFunction` or `FunctionExpression`
- `MethodDeclaration` (class methods, reported as `ClassName#methodName`)
- `ClassDeclaration` (named classes)

Anonymous inline callbacks are **not** report targets (but counted in `localFunctionCount` etc.).

## Output Fields

- `symbolName`: 表示用の識別子
- `symbolKind`: `function` / `variableFunction` / `method` / `class`
- `className`: class に属する symbol の親 class 名
- `memberName`: method / function の元の名前
- `functionName`: 後方互換のため残している旧フィールド（値は `symbolName` と同じ）

## Validation Update

次の検証では、`symbolKind=class` のレポートを起点に、class 単位の集約指標を
より明確に分離していく。

- constructor / public method / private method / accessor の内訳を見られるようにする
- class レベルの集約値（総 CC、平均 CC、最大 CC、判定ロジック集中度）を比較できるようにする
- 必要なら `functionName` を廃止して `symbolName` に一本化する

## Why Graphs Matter

現状の decision table は AST から `if / else / return` をたどって構築している。
この方式は軽量で説明しやすい一方、複雑な制御構造や関数間の判定連鎖を正確には扱えない。

そのため、次のフェーズでは AST 単独ではなく graph ベースの解析を入れる前提で考える。

- AST は構文の形を取るのに向いている
  predicate 抽出、ラベル生成、説明文生成の土台になる
- CFG (Control Flow Graph) は実際の制御フローを表現できる
  分岐、合流、早期 return、到達不能経路、ループを AST より正確に扱える
- Call Graph は関数 / メソッド間の呼び出し関係を表現できる
  `execute -> checkRisk` のような跨ぎを解析し、判断ロジックの分散や集中を追える

### Expected Benefits

- decision table の行が「本当に到達可能な経路か」を検証しやすくなる
- class / method をまたいだ判定ロジックのつながりを見える化できる
- dead branch、重複判定、早期 return 連鎖をより自然に要約できる
- 将来的に predicate 抽出と side-effect 分析を同じ経路上で結びつけやすくなる

### Recommended Order

graph を入れる順番は次のとおり。

1. AST + intra-procedural CFG
2. 複合条件の atomic condition 分解
3. 短絡評価を反映した MC/DC ケース生成
4. 必要な範囲に限定した call graph 導入

この順番にする理由は、call graph は効果が大きい一方で、
dynamic dispatch / callback / interface 越し呼び出し / async 境界で
一気に難しくなるため。

## Decision Table / MC/DC Positioning

現状の `--decision-table` は、厳密な形式検証としての MC/DC ではなく、
AST ベースの branch-sensitive な近似出力として扱う。

今の出力でできていること:

- method 単位の代表的な分岐経路を列挙する
- `T=true / F=false / *=not evaluated` の形で判定の流れを可視化する
- 各 predicate が outcome を変える代表ケースを出す

今の出力でまだできていないこと:

- 複合条件を完全な atomic condition に分解した厳密 MC/DC
- `&&` / `||` の短絡評価を反映した独立性証明
- CFG ベースの到達可能性検証
- Call graph を使った interprocedural な判定連鎖の解析

したがって、README / 実装 / 検証ログでは、現段階の MC/DC を
「strict MC/DC」ではなく「MC/DC-like witness pairs」または
「branch-sensitive approximation」として扱う。

## M1-M4 Evaluation

`samples/usecase/approveOrder.ts` に対して、次のコマンドで M1〜M4 相当を確認した。

```sh
# M1: basic metrics
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts

# M2: predicate extraction
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --predicates

# M3: effects / rule candidates
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --effects
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --rule-candidates

# M4: decision table / MC/DC-like output
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --decision-table
```

## Predicate Normalization

`--predicates` と `--decision-table` の出力で、raw な predicate テキストに加えて
**正規化名**（`normalizedName`）と**意味説明**（`trueMeaning`）を補助表示する。

### 目的

- `!order` → `orderMissing` のように、コードのまま出すより LLM に渡しやすい名前を付ける
- `operator.role !== 'admin'` → `operatorRoleNotAdmin` のように、人間にも読みやすい形にする
- `trueMeaning` / `falseMeaning` で、predicate が `true` / `false` のときの意味を言語化する

### 非ゴール

- **ソースコードのリネームは行わない**。正規化名は出力専用で、元のコードは変更しない。
- 完全な意味論的解析ではなく、構文パターンベースの近似。

### 対応パターン

| kind | 例 | normalizedName |
|---|---|---|
| truthiness | `user` | `userExists` |
| truthiness (negated) | `!order` | `orderMissing` |
| nullCheck | `order === null` | `orderNull` |
| nullCheck | `user !== undefined` | `userNotNull` |
| comparison (string) | `operator.role !== 'admin'` | `operatorRoleNotAdmin` |
| comparison (.length) | `errors.length > 0` | `errorsNonEmpty` |
| comparison (numeric) | `total >= HIGH_VALUE_THRESHOLD` | `totalHighValueExceeded` |
| call | `canTransition(order.status, 'approved')` | `canTransitionApprovedPassed` |
| call (negated) | `!canTransition(...)` | `canTransitionApprovedFailed` |

### 既知の限界

- 複合条件（`a && b`）は decision table 側でそのまま表示（分解は今後）
- `camelize` はキャメルケースの関数名の大文字を維持しない（例: `canTransition` → `cantransition`）
- 数値リテラル `0` はそのまま名前に入る（例: `itemQuantity0NotExceeded`）
- マッチしない predicate は `fallback` で `unknown_Condition` 形式になる

### Summary

- M1 は人間にも LLM にも有益
  複雑性の高い symbol を素早く見つける用途に向いている
- M2 は特に LLM に有益
  predicate を行番号付きで抽出できるため、説明生成やルール化の入力として使いやすい
- M3 の `--effects` は有益
  判定と副作用の位置関係が見えるため、人間にも LLM にも使いやすい
- M3 の `--rule-candidates` は現状ではやや荒い
  class 集約と method 個別の signal が混ざり、過検知気味になる
- M4 は現状でもっとも説明力が高い
  人間には分岐理解、LLM にはテスト観点抽出や要約入力として有益

### Human / LLM Value

- 人間向けに最も有益なのは M4
  条件と outcome の対応が直接見えるため、レビューや設計議論に使いやすい
- LLM 向けに安定して有益なのは M2 と M4
  predicate 一覧と decision table は、そのまま要約・分類・テスト生成の入力にしやすい
- M1 は要約用には有益だが、説明力は弱い
- M3 の `--rule-candidates` は、現状では補助情報として扱うのが妥当

## Improvement Ideas

M1〜M4 をより有益にするため、次の改善を優先する。

### M1

- class 専用メトリクスを分離する
  `methodCount`, `publicMethodCount`, `privateMethodCount`, `maxMethodCC`, `totalMethodCC`
- class 集約値と method 個別値を同じ指標で混ぜない

### M2

- predicate 名をコード断片のままではなく説明名に寄せる
  例: `!order` → `orderMissing`
- 複合条件を decision と atomic condition に分けて保持する
  例: `isPrivilegedOperator` と `roleIsAdmin` / `roleIsVip`

### M3

- `--effects` を predicate と同じ経路上で関連付けられるようにする
- `--rule-candidates` では class と method を別スコアリングにする
- 類似度判定で class 集約をそのまま method と比較しない

### M4

- AST 単独ではなく CFG を導入して到達可能性を検証する
- `&&` / `||` の短絡評価を反映した strict に近い MC/DC へ寄せる
- outcome ラベルを安定化し、return 文そのものへの依存を減らす
- call graph を限定導入し、`execute -> checkRisk` のような interprocedural な判定連鎖を扱えるようにする

## Proposal: LLM-Assisted Property Test Generation

これは実装済み機能ではなく提案である。

目的は、静的解析の結果をそのまま人間が読むだけで終わらせず、
LLM に渡して property test の `true / false` ケース generator と
テストコードの下書きを作らせ、さらに別段階でブラッシュアップできる形にすること。

### Basic Idea

大まかな流れは次のとおり。

1. 静的解析で predicate / decision / effect / decision table を抽出する
2. 正常系の baseline input と baseline mocks を用意する
3. predicate ごとの `true` / `false` 差分仕様を機械的に組み立てる
4. その中間表現を LLM に渡して generator と test の草案を生成する
5. 別プロンプトで LLM に命名、重複、assert、fixture 構造をブラッシュアップさせる

### Why Analysis Results Help

既存の分析結果は、LLM にとって次の点で有用。

- `predicates`
  どの条件を反転させたいか分かる
- `decision table`
  どの条件がどの outcome に結びつくか分かる
- `effects`
  成功時 / 失敗時にどの副作用が起こるべきか分かる
- `symbol metadata`
  どの function / method / class をテスト対象にするか分かる

特に M2 と M4 は、LLM に渡す素材として相性がよい。

### Information Needed

ただし、今の解析結果だけでは安定した property test generator を作るには足りない。
LLM が実用的なテストを組み立てるには、少なくとも次の情報が必要になる。

- 型情報
  input 引数、戻り値、依存先メソッド、union 値、optional / nullable 情報
- 値域制約
  例: `quantity > 0`, `unitPrice >= 0`, `status in ['active', 'suspended', ...]`
- baseline valid case
  全 guard を通る最小の正常入力
- baseline mocks
  repository / service の正常返却値
- predicate の正規化名
  例: `!order` ではなく `orderMissing`
- predicate の意味
  `true` が何を意味し、`false` が何を意味するか
- decision と atomic condition の分離
  `a && b` を 1 つの生文字列で終わらせず、必要なら atom に分ける
- expected outcome
  戻り値、例外、エラーメッセージ、成功 payload
- expected effects
  `save` が呼ばれる / 呼ばれない、呼び出し回数、保存内容
- domain invariants
  例: `saved.status === 'pending'`, `totalAmount >= 0`
- 既存テストスタイル
  `vitest`, `fast-check`, factory helper, mock helper の前提

### Recommended Intermediate Representation

LLM に渡すデータは自然言語だけでなく、JSON ベースの中間表現を持つのが望ましい。

最低限ほしい項目は次のとおり。

- `symbol`
- `symbolKind`
- `baselineInput`
- `baselineMocks`
- `predicates`
- `decisionRows`
- `trueCaseOverrides`
- `falseCaseOverrides`
- `expectedOutcome`
- `expectedEffects`
- `invariants`

### Example Shape

```json
{
  "symbol": "PlaceOrderUseCase#execute",
  "symbolKind": "method",
  "baselineInput": {
    "userId": "user-1",
    "items": [
      { "productId": "p1", "quantity": 1, "unitPrice": 1000 }
    ],
    "shippingAddress": {
      "postalCode": "100-0001",
      "prefecture": "Tokyo",
      "city": "Chiyoda",
      "street": "1-1"
    }
  },
  "baselineMocks": {
    "userRepo.findById": "activeUser",
    "orderRepo.save": "echoSavedOrder"
  },
  "predicates": [
    {
      "id": "P1",
      "name": "userExists",
      "raw": "!user",
      "trueMeaning": "user is missing",
      "falseMeaning": "user exists"
    }
  ],
  "cases": [
    {
      "predicateId": "P1",
      "falseCaseOverrides": {
        "mocks": { "userRepo.findById": "activeUser" }
      },
      "trueCaseOverrides": {
        "mocks": { "userRepo.findById": null }
      },
      "expectedOutcome": {
        "trueCase": "UserNotFound",
        "falseCase": "Continue"
      },
      "expectedEffects": {
        "trueCase": { "orderRepo.save.calls": 0 }
      }
    }
  ]
}
```

このような中間表現があれば、LLM は次のことを比較的安定して行える。

- `fast-check` の generator 作成
- predicate ごとの `true / false` witness case 生成
- property test の雛形出力
- fixture / factory の改善提案
- assertion の補強

### Suggested LLM Responsibilities

LLM にやらせる責務は、一度に全部ではなく段階を分けたほうがよい。

#### Stage 1: Generator Draft

- baseline input から差分で `true / false` ケースを作る
- mock override を作る
- property test generator の初稿を出す

#### Stage 2: Test Draft

- `vitest` / `fast-check` のテストコードを生成する
- outcome と side effect の assert を入れる
- 既存 helper に寄せて整形する

#### Stage 3: Review / Polish

- 重複を削る
- 命名を改善する
- overfit した case を減らす
- 境界値や shrink 方針を追加する

### What the LLM Should Not Infer Blindly

次の情報を LLM に推測で埋めさせるのは危険。

- repository の返却 shape
- 依存メソッドの副作用契約
- enum / union の完全な値域
- 「どのエラーを仕様として固定すべきか」
- property test の縮小戦略

これらはコードまたは中間表現として明示したほうがよい。

### Practical Benefits

この方式が成立すると、次のメリットがある。

- 人間は predicate と expected outcome をレビューすればよくなる
- LLM は生コード全体を毎回読むより安定して generator を組み立てられる
- decision table をテストケース生成に接続できる
- strict MC/DC に近づくための土台としても使える

### Risks / Caveats

- predicate 名が生コードのままだと、LLM の出力品質が安定しない
- baseline valid case がないと、複数条件を同時に壊してしまいやすい
- class 集約レポートは generator の直接入力としては不向き
  property test は method / function 単位を基本にしたほうがよい
- CFG / call graph がない段階では、到達不能ケースを混ぜる危険がある

### Recommended Next Step

実装を急がず提案段階で進めるなら、まずは次の順で検証するのがよい。

1. `method` 単位に限定する
2. baseline valid case を手で与える
3. predicate の正規化名を付ける
4. `true / false override spec` の JSON を設計する
5. その JSON を使って LLM に generator と test 雛形を書かせる

この順であれば、現在の AST ベース解析の延長として無理なく検証できる。

## Updated Milestones

### M1: Complexity Screening

目的:
複雑な function / method / class を素早く見つける入口をつくる。

主な出力:

- cyclomatic complexity
- nesting depth
- return count
- logical operator count
- class / method / function の区別

評価:

- 人間にも LLM にも有益
- ただし説明力は弱く、スクリーニング用途が中心

次の改善:

- class 専用メトリクスを分離する
- class 集約値と method 個別値を混ぜない

### M2: Predicate Extraction

目的:
条件分岐の中身を構造化し、後続の説明・ルール抽出・テスト生成の土台をつくる。

主な出力:

- atomic predicates
- negation count
- condition context (`if`, `elseif`, `ternary`, etc.)
- line numbers

評価:

- LLM 入力として非常に有益
- 要約、ルール候補化、テスト生成、命名補助の基礎になる

次の改善:

- predicate の正規化名を付ける
  例: `!order` → `orderMissing`
- decision と atomic condition を分離する
- short-circuit を意識した条件分解を導入する

### M3: Effect / Rule Hint Analysis

目的:
条件だけでなく、副作用とルール候補のシグナルも合わせて見られるようにする。

主な出力:

- effects (`dbRead`, `dbWrite`, `pureCall`, `return`, etc.)
- rule candidate signals
- repeated predicate detection
- decision convergence hints

評価:

- `effects` は人間にも LLM にも有益
- `rule-candidates` はまだ荒く、補助情報として扱うのが妥当

次の改善:

- class と method を別スコアリングにする
- class 集約を method 類似度判定に直接入れない
- predicate と effect を同一経路上で関連付ける

### M4: Decision Table Core

目的:
条件と outcome の対応を構造化し、説明・レビュー・テスト接続の中心成果物をつくる。

主な出力:

- decision table
- representative rows
- outcome labels
- MC/DC-like witness pairs

評価:

- 現状もっとも価値が高い出力
- 人間には分岐理解、LLM には要約・テスト観点抽出の入力として有益

次の改善:

- outcome ラベルを安定化する
- atomic condition と decision を分離する
- short-circuit を反映した stricter な分解に寄せる
- CFG を導入して到達可能性を検証する

### M5: LLM-Assisted Test Generation IR

目的:
解析結果を LLM に渡し、property test の true/false ケース generator と test draft を安定生成できる中間表現をつくる。

主な出力:

- baseline valid case
- baseline mocks
- normalized predicates
- true/false override spec
- expected outcome / expected effects
- invariants

想定フォーマット:

- JSON-based intermediate representation

評価:

- すでに研究対象として独立可能
- いきなり full 自動化ではなく、method 単位の小さな検証から始めるのが妥当

次の改善:

- method 単位に限定して試す
- baseline valid case を手で与える
- override spec を小さく設計する
- LLM に generator draft と test draft を書かせる

### M6: CFG / Call Graph Precision

目的:
AST ベース近似の限界を超え、到達可能性と関数間の判定連鎖をより正確に扱う。

主な出力:

- CFG ベースの decision path
- 到達不能経路の除外
- stricter MC/DC に近いケース生成
- 限定的 call graph による interprocedural analysis

評価:

- 精度改善の本命
- ただし難易度が高いため、M4 と M5 の価値を固めた後に進める

次の改善:

- まず intra-procedural CFG
- 次に short-circuit-aware MC/DC
- 最後に限定的 call graph

## Priority Summary

現時点の優先順位は次のとおり。

1. M4 の品質改善
2. M2 の predicate 正規化
3. M3 の class / method 分離
4. M5 の小規模検証
5. M6 の CFG 導入

## Working Assumption

この POC の価値の中心は、単なる複雑度計測ではなく、
「条件と outcome の対応を構造化して見せること」にある。

そのため、今後の設計の主軸は M4 を中心に置き、
M2 を基礎データ、M3 を補助、M5 を LLM 連携先、M6 を精度改善として育てる。

## Constraints (POC scope)

- Current implementation reports named functions, variable functions, class methods, and classes
- Syntax-based only (no type information used)
- No interprocedural analysis
- Decision table generation is AST-based and method-local
- Callback classification is structural, not semantic
- Metrics are heuristics to assist design decisions
- Predicate normalization is output-only (no source code renaming)

# CLI Guide

このドキュメントは `src/cli` 配下の TypeScript 製 CLI を実際に使ったときの目的、使い方、出力の意味をまとめたものです。
README の概要説明とは分けて、CLI 利用時の実務向けガイドとして書いています。

現在の主実装は Rust / Oxc ベースの [`rust-analyzer/README.md`](../rust-analyzer/README.md) です。このガイドは legacy / prototype 扱いの Node CLI 向けです。

## CLI 一覧

`src/cli` 配下にある CLI は次の 2 つです。

- `src/cli/analyze.ts`
  構文ベースの静的解析 CLI。複雑度、predicate、effects、decision table、test template を出します。
- `src/cli/llm-analyze.ts`
  静的解析結果を下敷きにして Claude Code agent に設計レビューを書かせる CLI です。

## 事前条件

- Node.js と `npm install` 済みであること
- `npx tsx ...` で TypeScript CLI を直接実行できること
- `llm-analyze.ts` は `@anthropic-ai/claude-agent-sdk` と外部エージェント実行環境が必要です

## 1. `analyze.ts`

### 目的

TypeScript の function / method / class を構文的に解析して、次の観点を出します。

- 複雑度の高い symbol を見つける
- 条件式を取り出す
- 副作用を見つける
- decision table を作る
- MC/DC-like な witness case の材料を作る
- テスト雛形の叩き台を作る

### 基本形

```sh
npx tsx src/cli/analyze.ts <file.ts> [options]
```

### `--json` が使えるか

- `analyze.ts` は `--json` をサポートしています
- `llm-analyze.ts` は `--json` をサポートしていません
- `analyze.ts` でも `--json` は `--debug` / `--predicates` / `--effects` / `--rule-candidates` と同時には使えません

### よく使うオプション

| Option | 目的 |
|---|---|
| `--json` | 生の JSON を出す |
| `--top <n>` | 複雑度上位 N 件だけ出す |
| `--min-complexity <n>` | CC が一定以上の symbol だけ出す |
| `--debug` | AST を表示する |
| `--debug-depth <n>` | AST の表示深さを制限する |
| `--predicates` | 抽出した predicate を出す |
| `--effects` | 副作用や return を出す |
| `--rule-candidates` | ルール候補シグナルを出す |
| `--decision-table` | decision table と MC/DC-like rows を出す |
| `--test-template` | decision table から vitest テンプレートを出す |

### 1-1. デフォルト出力

#### コマンド

```sh
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts
```

#### 代表出力

```text
[class] ApproveOrderUseCase  (.../approveOrder.ts:20)
  CC=10  nestDepth=2  fnNest=2  cbDepth=1
  if=9  elseif=0  switch=0  ternary=0  return=10

[method] ApproveOrderUseCase#execute  (.../approveOrder.ts:26)
  CC=7  nestDepth=2  fnNest=0  cbDepth=0
```

#### 何が分かるか

- どの symbol が重いか
- `class` と `method` のどちらが複雑さの主因か
- 分岐数、return 数、ネスト深さがどれくらいか

#### 向いている用途

- 最初のスクリーニング
- レビュー対象の絞り込み
- LLM に渡す前の粗い選別

### 1-2. JSON 出力

#### コマンド

```sh
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --json
```

#### 代表出力

```json
[
  {
    "symbolName": "ApproveOrderUseCase#execute",
    "symbolKind": "method",
    "className": "ApproveOrderUseCase",
    "memberName": "execute",
    "functionName": "ApproveOrderUseCase#execute",
    "filePath": "/Users/.../samples/usecase/approveOrder.ts",
    "startLine": 26,
    "metrics": {
      "ifCount": 6,
      "elseIfCount": 0,
      "switchCount": 0,
      "ternaryCount": 0,
      "returnCount": 6,
      "maxNestingDepth": 2,
      "cyclomaticComplexity": 7,
      "localFunctionCount": 0,
      "functionNestingDepth": 0,
      "callbackNestingDepth": 0,
      "logicalOperatorCount": 1,
      "negationCount": 5,
      "atomicConditionCount": 7,
      "maxConditionDepth": 1
    },
    "predicates": [
      {
        "text": "order",
        "negated": true,
        "kind": "truthiness",
        "context": "if",
        "line": 28,
        "normalizedName": "orderMissing",
        "trueMeaning": "order is missing / falsy",
        "falseMeaning": "order exists"
      }
    ],
    "effects": [
      {
        "kind": "call",
        "sideEffect": "dbRead",
        "text": "const order = await this.orderRepo.findById(input.orderId)",
        "line": 27
      }
    ]
  }
]
```

#### 何が分かるか

- `symbolName`
- `symbolKind`
- `metrics`
- `predicates`
- `effects`

#### フィールドの見方

- `symbolName`
  表示用の識別子です。method なら `ClassName#methodName` の形になります。
- `symbolKind`
  `function` / `variableFunction` / `method` / `class` のどれかです。
- `className`, `memberName`
  method や class に属する symbol の補助情報です。
- `metrics`
  複雑度や条件式数の集計です。
- `predicates`
  抽出した条件式です。`text` は raw、`normalizedName` は説明用の補助名です。
- `effects`
  return、call、dbRead、dbWrite などの副作用・制御終了情報です。

#### JSON が特に有効な場面

- 他ツールにそのまま入力したいとき
- LLM に自然言語ではなく構造化データを渡したいとき
- property test 用の中間 JSON を作る前段にしたいとき
- レポートを保存して後処理したいとき

#### 組み合わせ例

```sh
# 複雑度 5 以上の symbol だけ JSON で出す
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --min-complexity 5 --json

# 上位 2 件だけ JSON で出す
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --top 2 --json
```

#### 向いている用途

- 他ツール連携
- JSON ベースの中間表現づくり
- LLM への機械入力

### 1-3. `--top` / `--min-complexity`

#### コマンド例

```sh
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --top 1
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --min-complexity 5
```

#### 何が分かるか

- 重要そうな symbol だけに絞って見られる
- class / method が多いファイルでもレビュー対象を減らせる

#### 向いている用途

- 大きいファイルの優先順位付け
- LLM コスト節約

### 1-4. `--debug`

#### コマンド

```sh
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --debug --debug-depth 2 --top 1
```

#### 代表出力

```text
[class] ApproveOrderUseCase  :20
  [AST]  markers: [CC+1] cyclomatic +1 / [nest] nesting depth +1 / [fn] nested function
  ClassDeclaration
    Identifier: "ApproveOrderUseCase"
    MethodDeclaration ƒ[fn]
    MethodDeclaration ƒ[fn]
```

#### 何が分かるか

- どの AST ノードが複雑度に効いているか
- class / method の入れ子構造
- 正規化や decision table の元になる構文の形

#### 向いている用途

- 実装デバッグ
- 指標の根拠確認

### 1-5. `--predicates`

#### コマンド

```sh
npx tsx src/cli/analyze.ts samples/usecase/placeOrder.ts --predicates
```

#### 代表出力

```text
[method] PlaceOrderUseCase#execute  (.../placeOrder.ts:32)
  ! [truthiness ] [if    ] :34  user
       → userMissing  true="user is missing / falsy"
    [comparison ] [if    ] :41  input.items.length === 0
       → inputItemsEmpty  true="input.items.length is 0"
    [comparison ] [if    ] :45  item.quantity <= 0
       → itemQuantityZeroOrBelow  true="item.quantity <= 0"
```

#### 何が分かるか

- 条件式の raw 形
- 種別 (`truthiness`, `comparison`, `call` など)
- 行番号
- 正規化名 (`normalizedName`)
- `true` のときの意味 (`trueMeaning`)

#### 向いている用途

- LLM に渡す条件一覧づくり
- predicate 正規化の確認
- ルール名候補の抽出

### 1-6. `--effects`

#### コマンド

```sh
npx tsx src/cli/analyze.ts samples/usecase/placeOrder.ts --effects
```

#### 代表出力

```text
[method] PlaceOrderUseCase#execute  (.../placeOrder.ts:32)
  [call      ][dbRead     ] :33  const user = await this.userRepo.findById(input.userId)
  [return    ][return     ] :35  return { ok: false, error: 'User not found' }
  [call      ][pureCall   ] :58  const subtotal = calcOrderTotal(input.items)
  [call      ][dbWrite    ] :82  const saved = await this.orderRepo.save(order)
```

#### 何が分かるか

- どこで DB read / write が起きるか
- どこで pureCall が行われるか
- どこで return / throw しているか

#### 向いている用途

- 副作用の位置把握
- 失敗時に何が起きないべきかの確認
- テスト assert の材料づくり

### 1-7. `--rule-candidates`

#### コマンド

```sh
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --rule-candidates
```

#### 代表出力

```text
Rule Candidate Analysis  (3 signals found)

[score=10] [method] ApproveOrderUseCase#execute  :26
  signals: highComplexity, repeatedPredicate, tenantBranch, decisionConvergence, similarFunction
  • Repeated predicates (7): order, operator, operator.role !== admin
```

#### 何が分かるか

- ルールエンジン化や共通化の候補
- predicate の重複
- role / tenant 系分岐
- return 形の収束

#### 向いている用途

- ルール抽出候補の洗い出し
- 設計改善議論のたたき台

#### 注意

- class 集約と method 個別が混ざるため、現状は補助情報として見るのが安全です

### 1-8. `--decision-table`

#### コマンド

```sh
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --decision-table
```

#### 代表出力

```text
[method] ApproveOrderUseCase#execute  :26  (6 decisions)
  T=true  F=false  *=not evaluated

  Predicates
  P1  :28  !order  → orderMissing
  P2  :33  !operator  → operatorMissing

  Rows
  P1   P2   P3   P4   P5   P6    outcome
  T    *    *    *    *    *     OrderNotFound
  F    F    F    F    T    F     Success ← success path
```

#### 何が分かるか

- どの条件が outcome を変えるか
- 代表的な到達条件
- success path と failure path の違い
- MC/DC-like な `true / false` witness case

#### 向いている用途

- 人間向けレビュー
- LLM 向け要約入力
- property test / test template の前処理

### 1-9. `--test-template`

#### コマンド

```sh
npx tsx src/cli/analyze.ts samples/usecase/approveOrder.ts --test-template
```

#### 代表出力

```ts
describe('ApproveOrderUseCase#execute', () => {
  it('should distinguish predicate "!order" switching between false and true', () => {
    // Arrange false-case: !order = false
    // Arrange true-case:  !order = true
  })
})
```

#### 何が分かるか

- decision table をテストの形に落としたらどうなるか
- どの predicate に対してどんな false/true ケースが必要か

#### 向いている用途

- vitest テンプレートの叩き台
- property test IR 設計の参考

#### 注意

- そのまま実行可能な完成テストではなく、あくまで draft です

## 2. `llm-analyze.ts`

### 目的

静的解析の結果を下敷きにして、Claude Code agent に設計レビューを生成させます。

### 基本形

```sh
npx tsx src/cli/llm-analyze.ts <file.ts> [options]
```

### オプション

| Option | 目的 |
|---|---|
| `--symbol <name>` | 特定 symbol のみ対象にする |
| `--min-complexity <n>` | CC が一定以上の symbol だけ対象にする |
| `--top <n>` | 上位 N 件だけ対象にする |

### 実行例

```sh
npx tsx src/cli/llm-analyze.ts samples/usecase/placeOrder.ts --min-complexity 5
```

### 実際に確認できた出力

```text
[method] PlaceOrderUseCase#execute  :32  CC=8
Spawning Claude Code agent...
```

このコマンドは外部エージェント応答待ちになるため、出力本文は環境と応答時間に依存します。

### 出力で何をするか

`llm-analyze.ts` は、まず内部で静的解析を行い、その結果をプロンプトに埋め込んだうえで Claude Code agent に次を依頼します。

- Facts
- Decisions
- Actions
- Rule Extraction Opportunities
- Design Notes

### 何が分かるか

- この method を業務ルールとして読むと何が事実で何が判定か
- どこがルール抽出候補か
- どこが責務過多か
- 命名や設計分割の改善案

### 向いている用途

- ドメイン設計レビューの下書き
- 人間向け説明文の生成
- ルール抽出候補の議論

### 注意

- これは静的解析結果そのものではなく、LLM の解釈付きレビューです
- 厳密な事実確認は `analyze.ts` 側の構造化出力で行うべきです

## 3. どの CLI をいつ使うか

### まず見る

- `analyze.ts`
  複雑な symbol を見つけたいとき

### 条件を理解したい

- `analyze.ts --predicates`
  predicate 一覧と正規化名を見たいとき

### 副作用を理解したい

- `analyze.ts --effects`
  DB read / write や return を見たいとき

### ルール候補を探したい

- `analyze.ts --rule-candidates`
  ただし補助情報扱いが安全

### 分岐と outcome の対応を見たい

- `analyze.ts --decision-table`
  現状もっとも説明力が高い出力

### テストの叩き台を作りたい

- `analyze.ts --test-template`

### 人間向けレビュー文がほしい

- `llm-analyze.ts`

## 4. 現時点でのおすすめ導線

実務で使うなら、次の順番が分かりやすいです。

1. `analyze.ts --min-complexity 5`
2. `analyze.ts --predicates`
3. `analyze.ts --effects`
4. `analyze.ts --decision-table`
5. 必要なら `analyze.ts --test-template`
6. 人間向けレビューが欲しければ `llm-analyze.ts`

この順番にすると、

- M1 で対象を絞り
- M2 / M3 で構造を見て
- M4 で説明可能な形にし
- その後に LLM やテスト生成へつなぐ

という流れになります。

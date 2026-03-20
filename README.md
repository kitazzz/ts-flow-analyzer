# recast-forge POC: TypeScript Function/Class Complexity Analyzer

TypeScript ファイルの関数 / メソッドごとに複雑性メトリクスを計測する CLI ツール。

現状は `function` / `variableFunction` / `method` / `class` をレポート対象とし、
出力では `symbolKind` で種別を区別できる。

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

## Constraints (POC scope)

- Current implementation reports named functions, variable functions, class methods, and classes
- Syntax-based only (no type information used)
- No interprocedural analysis
- Decision table generation is AST-based and method-local
- Callback classification is structural, not semantic
- Metrics are heuristics to assist design decisions
- Predicate extraction and side-effect classification are out of scope (next phase)

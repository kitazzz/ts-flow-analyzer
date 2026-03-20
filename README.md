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

## Constraints (POC scope)

- Current implementation reports named functions, variable functions, class methods, and classes
- Syntax-based only (no type information used)
- No interprocedural analysis
- Callback classification is structural, not semantic
- Metrics are heuristics to assist design decisions
- Predicate extraction and side-effect classification are out of scope (next phase)

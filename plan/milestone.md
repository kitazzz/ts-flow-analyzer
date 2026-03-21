# Roadmap: TypeScript Complexity / Rule Extraction Analyzer

## Overview

TypeScript コードベースに対して静的解析を行い、関数ごとの複雑さ、条件分岐の構造、ルールエンジン化候補、さらに将来的な真理値表 / MC/DC / プロパティテスト生成までつなげる analyzer を構築する。

当面は `ts-morph` を用いて方向性を固め、その後必要に応じて Rust / Oxc 系へ移行する。

---

## Current Strategy

- まずは `ts-morph` で探索・仕様固めを行う
- いきなり Rust へは行かない
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

### 4. ts-morph は探索フェーズ
当面は `ts-morph` で
- 欲しい出力
- 効くメトリクス
- 抽出すべき構造
を固める。

### 5. Rust / Oxc は後続
以下が必要になったら Rust / Oxc へ進む:
- AST walk では限界
- intraprocedural CFG が必要
- function summary が必要
- ts-morph では性能不足
- analyzer core を独立させたい

---

## Near-term Priority

1. Milestone 1 を安定化
2. Milestone 2 で条件式と効果を抽出
3. Milestone 3 で rule candidate を見つける
4. Milestone 4 で truth table / MC/DC / property test
5. Milestone 5 で LLM handoff

---

## Non-goals for Now

現時点では以下はやらない:
- 完全な interprocedural analysis
- 本格的な CFG / dataflow engine
- Rust 実装
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

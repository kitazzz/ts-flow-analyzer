# ts-flow-analyzer

TypeScript / JavaScript を対象に、single-file 静的解析を行う Rust / Oxc ベースの実験プロジェクトです。

このリポジトリは Rust 実装を主系としており、旧 `ts-morph` / Node ベース実装は削除済みです。

## Quick Start

```sh
cargo install --path analyzer --force

ts-flow-analyzer samples/usecase/approveOrder.ts --all --json
ts-flow-analyzer samples/usecase/approveOrder.ts --data-flow --json
ts-flow-analyzer samples/usecase/approveOrder.ts --graph --data-flow
```

## Main Outputs

- metrics
- predicates
- effects
- local def-use / data-flow
- decision table / MC/DC-like cases
- intra-file call graph
- unified Graph IR / DOT

## Repository Layout

- [`analyzer/`](analyzer/)
  Rust crate 本体。CLI、解析器、Graph IR 実装が入っています。
- [`samples/`](samples/)
  動作確認用の TypeScript サンプルです。
- [`docs/REPOSITORY_LAYOUT.md`](docs/REPOSITORY_LAYOUT.md)
  リポジトリ構成と整理方針です。
- [`plan/milestone.md`](plan/milestone.md)
  マイルストーンと中期計画です。

## Main Docs

- CLI と使い方: [`docs/CLI.md`](docs/CLI.md)
- 実装構成とデータモデル: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- フェーズ別の実装解説: [`docs/IMPLEMENTATION_PHASES.md`](docs/IMPLEMENTATION_PHASES.md)
- ICFG / SDG / CPG 実装計画: [`docs/ICFG_SDG_CPG_PLAN.md`](docs/ICFG_SDG_CPG_PLAN.md)
- リポジトリ構成: [`docs/REPOSITORY_LAYOUT.md`](docs/REPOSITORY_LAYOUT.md)

## Notes

- 現在の解析スコープは single-file / intraprocedural を中心にしています。
- `cfg-analysis` feature を付けたビルドでは CFG 系の出力が増えます。
- 次フェーズは ICFG / SDG / CPG 拡張です。

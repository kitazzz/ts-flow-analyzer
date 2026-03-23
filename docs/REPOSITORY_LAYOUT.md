# Repository Layout

このリポジトリは現在、Rust / Oxc ベースの実装を主系として整理しています。

旧 `ts-morph` / Node ベースの試作実装は削除し、トップレベルは次の責務に絞っています。

## Top Level

```text
.
├── .cargo/
├── README.md
├── analyzer/
├── docs/
├── plan/
└── samples/
```

## Directories

- `analyzer/`
  実装本体です。Cargo crate、CLI、解析器、Graph IR、設計書が入ります。
- `samples/`
  手動確認や回帰確認に使う TypeScript サンプルです。
- `docs/`
  リポジトリ全体のドキュメント置き場です。CLI、設計、実装フェーズ、構成説明をここに集約しています。
- `plan/`
  マイルストーンと実装計画です。
- `.cargo/config.toml`
  Cargo の共有設定です。build artifact の出力先を repo root の `target/` に固定しています。

## Why `analyzer/` Stays

crate をリポジトリ root へ移すこともできますが、今の時点では次の理由で分けています。

- root に `samples/`, `docs/`, `plan/` を置いたまま整理しやすい
- ソースコードは `analyzer/` に寄せつつ、build artifact は repo root の `target/` にまとめられる
- 既存ドキュメントとコマンド導線の変更範囲を必要以上に広げなくて済む

必要になれば将来 multi-crate 化や root 直下への統合を検討しますが、現時点では優先度は高くありません。

## Removed

今回の整理で次を削除しています。

- legacy `ts-morph` analyzer 実装
- legacy Node CLI
- `package.json` / `package-lock.json` / `tsconfig.json`
- legacy CLI 専用ドキュメント

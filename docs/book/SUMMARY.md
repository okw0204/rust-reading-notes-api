# 目次

- [教材の使い方](introduction.md)

# 第1部：処理の流れを読む

- [依存関係を組み立てる](01-flow/composition.md)
- [本の登録を端から端まで追う](01-flow/create-book.md)
- [詳細取得とエラーの出口](01-flow/detail-and-errors.md)

# 第2部：型の保証と実行時の境界

- [検証済みの文字列を値型にする](02-types/validated-values.md)
- [型状態で遷移を制限する](02-types/typestate.md)
- [実行時の状態を型へ接続する](02-types/runtime-boundaries.md)

# 第3部：抽象化と非同期を読む

- [永続化の契約を trait で読む](03-abstraction/repository-trait.md)
- [ジェネリックな service を具体化する](03-abstraction/generic-service.md)
- [非同期処理の借用と共有を読む](03-abstraction/async-bounds.md)

# 第4部：テストから保証を読む

- [フェイクで service の判断を確かめる](04-tests/service-fake.md)
- [SQLite で保存の契約を確かめる](04-tests/sqlite-contract.md)
- [Router から HTTP の保証を確かめる](04-tests/http-integration.md)

# 目次

- [教材の使い方](introduction.md)

# 第 1 部：値と関数

- [値を受け取る関数を読む](01-values/values-and-functions.md)
- [本の登録で所有権を追う](01-values/create-book.md)
- [要求から検証済みの入力へ](01-values/reading-completion-input.md)
- [成立する実装と別案を比べる](01-values/compare-alternatives.md)

# 第 2 部：型と状態

- [検証済みの文字列を値型にする](02-types/validated-values.md)
- [型状態で遷移を制限する](02-types/typestate.md)
- [実行時の状態を型へ接続する](02-types/runtime-boundaries.md)
- [失敗後に何が残るか](02-types/completion-failures.md)

# 第 3 部：型を抽象化する

- [永続化の契約を trait で読む](03-abstraction/repository-trait.md)
- [ジェネリックな service を具体化する](03-abstraction/generic-service.md)

# 第 4 部：非同期と共有

- [非同期処理の借用と共有を読む](04-async/async-bounds.md)
- [独立した読了記録を並行に進める](04-async/concurrent-completions.md)
- [完了順と入力順を分ける](04-async/completion-order.md)

# 第 5 部：全体を読み直す

- [状態変更を HTTP から SQLite まで追う](05-flow/status-update.md)
- [フェイクで service の判断を確かめる](05-flow/service-fake.md)
- [SQLite で保存の契約を確かめる](05-flow/sqlite-contract.md)
- [Router から HTTP の保証を確かめる](05-flow/http-integration.md)

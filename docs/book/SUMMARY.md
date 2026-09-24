# 目次

- [完成形と一括読了記録](introduction.md)

# 第 1 部：一件の値を整える

- [要求から検証済みの入力へ](01-values/reading-completion-input.md)
- [借用して検査し、所有して渡す](01-values/borrow-and-own.md)

# 第 2 部：一冊の読了を成立させる

- [保存状態を型状態へ接続する](02-types/stored-state-to-typestate.md)
- [状態変更とメモを一緒に保存する](02-types/atomic-completion.md)
- [失敗後に何が残るか](02-types/completion-failures.md)

# 第 3 部：Interface の向こうを読む

- [`BookRepository` の契約](03-abstraction/repository-trait.md)
- [フェイクと SQLite Adapter](03-abstraction/adapters.md)

# 第 4 部：複数の処理を進める

- [Future が値を保持する範囲](04-async/future-values.md)
- [独立した読了記録を並行に進める](04-async/concurrent-completions.md)
- [完了順と入力順を分ける](04-async/completion-order.md)
- [失敗と親 Future の終了](04-async/failure-and-parent-future.md)

# 第 5 部：利用者の経路を読み直す

- [HTTP から SQLite まで](05-flow/http-to-sqlite.md)
- [検証の保証と限界](05-flow/verification-limits.md)

use super::*;

use crate::test_support::FakeBookRepository;

#[tokio::test]
async fn fake_conditional_update_rejects_a_stale_state() {
    let repository = FakeBookRepository::default();
    let title = BookTitle::try_from("Book".to_owned()).unwrap();
    let author = Author::try_from("Author".to_owned()).unwrap();
    let current = repository.insert_book(&title, &author).await.unwrap();
    let first = repository.find_book(current.id()).await.unwrap();
    let second = repository.find_book(current.id()).await.unwrap();
    let StoredBook::WantToRead(first) = first else {
        panic!("取得した書籍は未読")
    };
    let StoredBook::WantToRead(second) = second else {
        panic!("二度目に取得した書籍も未読")
    };
    let saved = repository
        .update_book_status(
            ReadingStatus::WantToRead,
            StoredBook::Reading(first.start_reading()),
        )
        .await
        .unwrap();
    assert_eq!(saved.status(), ReadingStatus::Reading);
    // エラー注入を使わず、SQLite と同じ条件付き更新の契約を確かめる。
    let error = repository
        .update_book_status(
            ReadingStatus::WantToRead,
            StoredBook::Reading(second.start_reading()),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Conflict));
    assert_eq!(
        repository.find_book(current.id()).await.unwrap().status(),
        ReadingStatus::Reading
    );
}

#[tokio::test]
async fn fake_conditional_update_rejects_deletion_after_fetch() {
    let repository = FakeBookRepository::default();
    let title = BookTitle::try_from("Book".to_owned()).unwrap();
    let author = Author::try_from("Author".to_owned()).unwrap();
    let current = repository.insert_book(&title, &author).await.unwrap();
    let fetched = repository.find_book(current.id()).await.unwrap();
    let StoredBook::WantToRead(book) = fetched else {
        panic!("取得した書籍は未読")
    };
    repository.delete_book(current.id()).await.unwrap();
    let error = repository
        .update_book_status(
            ReadingStatus::WantToRead,
            StoredBook::Reading(book.start_reading()),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Conflict));
    assert!(matches!(
        repository.find_book(current.id()).await,
        Err(AppError::NotFound)
    ));
}

// ANCHOR: service_conflict_test
#[tokio::test]
async fn reports_a_save_conflict_without_changing_the_book() {
    let fake = FakeBookRepository::default();
    let service = ReadingService::new(fake.clone());
    let book = service
        .create_book(CreateBook {
            title: "Rust Book".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    fake.fail_next_update(AppError::Conflict);
    let error = service
        .update_status(
            book.id(),
            UpdateStatus {
                status: "reading".to_owned(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Conflict));
    let stored = fake.find_book(book.id()).await.unwrap();
    assert_eq!(stored.status(), ReadingStatus::WantToRead);
    // 保存エラーは一度だけ返し、再試行では正しい遷移を保存できる。
    assert_eq!(
        service
            .update_status(
                book.id(),
                UpdateStatus {
                    status: "reading".to_owned(),
                }
            )
            .await
            .unwrap()
            .status(),
        ReadingStatus::Reading
    );
}

// ANCHOR_END: service_conflict_test

#[tokio::test]
async fn rejects_invalid_inputs_without_changing_saved_state() {
    let service = ReadingService::new(FakeBookRepository::default());
    for (title, author) in [(" ", "Author"), ("Title", "\t")] {
        assert!(matches!(
            service
                .create_book(CreateBook {
                    title: title.to_owned(),
                    author: author.to_owned(),
                })
                .await,
            Err(AppError::Validation(_))
        ));
        assert!(service.list_books(None).await.unwrap().is_empty());
    }

    let book = service
        .create_book(CreateBook {
            title: "Book".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    assert!(matches!(
        service
            .add_note(
                book.id(),
                AddNote {
                    body: "\n".to_owned(),
                }
            )
            .await,
        Err(AppError::Validation(_))
    ));
    assert!(service.get_book(book.id()).await.unwrap().notes.is_empty());

    assert!(matches!(
        service.list_books(Some("paused".to_owned())).await,
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        service
            .update_status(
                book.id(),
                UpdateStatus {
                    status: "paused".to_owned(),
                }
            )
            .await,
        Err(AppError::Validation(_))
    ));
    assert_eq!(
        service.get_book(book.id()).await.unwrap().book.status(),
        ReadingStatus::WantToRead
    );
}

#[tokio::test]
async fn preserves_a_database_save_error_and_the_stored_state() {
    let fake = FakeBookRepository::default();
    let service = ReadingService::new(fake.clone());
    let book = service
        .create_book(CreateBook {
            title: "Book".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    fake.fail_next_update(AppError::Database(sqlx::Error::PoolClosed));
    assert!(matches!(
        service
            .update_status(
                book.id(),
                UpdateStatus {
                    status: "reading".to_owned(),
                }
            )
            .await,
        Err(AppError::Database(sqlx::Error::PoolClosed))
    ));
    assert_eq!(
        service.get_book(book.id()).await.unwrap().book.status(),
        ReadingStatus::WantToRead
    );
}

#[tokio::test]
async fn rejects_invalid_transitions_without_changing_saved_state() {
    let service = ReadingService::new(FakeBookRepository::default());
    let book = service
        .create_book(CreateBook {
            title: "Book".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    for (advance, rejected, expected) in [
        (
            None,
            vec!["want_to_read", "finished"],
            ReadingStatus::WantToRead,
        ),
        (
            Some("reading"),
            vec!["want_to_read", "reading"],
            ReadingStatus::Reading,
        ),
        (
            Some("finished"),
            vec!["want_to_read", "reading", "finished"],
            ReadingStatus::Finished,
        ),
    ] {
        if let Some(status) = advance {
            service
                .update_status(
                    book.id(),
                    UpdateStatus {
                        status: status.to_owned(),
                    },
                )
                .await
                .unwrap();
        }
        for status in rejected {
            assert!(matches!(
                service
                    .update_status(
                        book.id(),
                        UpdateStatus {
                            status: status.to_owned(),
                        }
                    )
                    .await,
                Err(AppError::Conflict)
            ));
            assert_eq!(
                service.get_book(book.id()).await.unwrap().book.status(),
                expected
            );
        }
    }
}

#[tokio::test]
async fn manages_books_and_notes_through_the_repository() {
    let service = ReadingService::new(FakeBookRepository::default());
    let first = service
        .create_book(CreateBook {
            title: " Book \n".to_owned(),
            author: " Author \t".to_owned(),
        })
        .await
        .unwrap();
    let second = service
        .create_book(CreateBook {
            title: "Second".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(first.status(), ReadingStatus::WantToRead);
    let (_, title, author, _) = first.clone().into_parts();
    assert_eq!(title.as_str(), "Book");
    assert_eq!(author.as_str(), "Author");
    assert!(service.get_book(first.id()).await.unwrap().notes.is_empty());
    let note = service
        .add_note(
            first.id(),
            AddNote {
                body: " memo \n".to_owned(),
            },
        )
        .await
        .unwrap();
    let detail = service.get_book(first.id()).await.unwrap();
    assert_eq!(detail.notes.len(), 1);
    assert_eq!(detail.notes[0].id, note.id);
    assert_eq!(detail.notes[0].body.as_str(), "memo");
    service
        .update_status(
            first.id(),
            UpdateStatus {
                status: "reading".to_owned(),
            },
        )
        .await
        .unwrap();
    let all = service.list_books(None).await.unwrap();
    assert_eq!(
        all.iter().map(StoredBook::id).collect::<Vec<_>>(),
        vec![first.id(), second.id()]
    );
    let filtered = service
        .list_books(Some("reading".to_owned()))
        .await
        .unwrap();
    assert_eq!(
        filtered.iter().map(StoredBook::id).collect::<Vec<_>>(),
        vec![first.id()]
    );
    service.delete_book(first.id()).await.unwrap();
    assert!(matches!(
        service.get_book(first.id()).await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        service.delete_book(first.id()).await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        service
            .add_note(
                first.id(),
                AddNote {
                    body: "memo".to_owned()
                }
            )
            .await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        service
            .update_status(
                first.id(),
                UpdateStatus {
                    status: "reading".to_owned()
                }
            )
            .await,
        Err(AppError::NotFound)
    ));
}

#[tokio::test]
async fn advances_completions_concurrently_and_returns_results_in_input_order() {
    use std::time::Duration;

    let fake = FakeBookRepository::default();
    let service = ReadingService::new(fake.clone());
    let first = service
        .create_book(CreateBook {
            title: "First".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    let second = service
        .create_book(CreateBook {
            title: "Second".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    for book_id in [first.id(), second.id()] {
        service
            .update_status(
                book_id,
                UpdateStatus {
                    status: "reading".to_owned(),
                },
            )
            .await
            .unwrap();
    }

    let control = fake.control_reading_completions([first.id(), second.id()]);
    let run = service.record_reading_completions(RecordReadingCompletions {
        items: vec![
            RecordReadingCompletion {
                book_id: first.id(),
                body: "first note".to_owned(),
            },
            RecordReadingCompletion {
                book_id: second.id(),
                body: "second note".to_owned(),
            },
        ],
    });
    let drive = async {
        control.wait_for_started(2).await;
        assert!(control.finished().is_empty());

        control.release(second.id());
        control.wait_for_finished(1).await;
        assert_eq!(control.finished(), vec![second.id()]);
        control.release(first.id());
    };

    let (results, ()) =
        tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(run, drive) })
            .await
            .expect("both completions should start before either is released");
    let result_ids = results
        .unwrap()
        .into_iter()
        .map(|result| match result {
            ReadingCompletionResult::Completed(completion) => completion.book.id(),
            ReadingCompletionResult::Failed { book_id, .. } => {
                panic!("book {} unexpectedly failed", book_id.0)
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(result_ids, vec![first.id(), second.id()]);
}

#[tokio::test]
async fn continues_other_completions_after_one_fails() {
    use std::time::Duration;

    let fake = FakeBookRepository::default();
    let service = ReadingService::new(fake.clone());
    let first = service
        .create_book(CreateBook {
            title: "First".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    let second = service
        .create_book(CreateBook {
            title: "Second".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    for book_id in [first.id(), second.id()] {
        service
            .update_status(
                book_id,
                UpdateStatus {
                    status: "reading".to_owned(),
                },
            )
            .await
            .unwrap();
    }

    let control = fake.control_reading_completions([first.id(), second.id()]);
    fake.fail_reading_completion(first.id(), AppError::Database(sqlx::Error::PoolClosed));
    let run = service.record_reading_completions(RecordReadingCompletions {
        items: vec![
            RecordReadingCompletion {
                book_id: first.id(),
                body: "first note".to_owned(),
            },
            RecordReadingCompletion {
                book_id: second.id(),
                body: "second note".to_owned(),
            },
        ],
    });
    let drive = async {
        control.wait_for_started(2).await;
        control.release(first.id());
        control.wait_for_finished(1).await;
        assert_eq!(control.finished(), vec![first.id()]);
        control.release(second.id());
    };

    let (results, ()) =
        tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(run, drive) })
            .await
            .expect("the failure should not stop the other completion");
    let results = results.unwrap();
    assert!(matches!(
        &results[0],
        ReadingCompletionResult::Failed {
            book_id,
            error: ReadingCompletionFailure::Internal,
        } if *book_id == first.id()
    ));
    assert!(matches!(
        &results[1],
        ReadingCompletionResult::Completed(completion)
            if completion.book.id() == second.id()
    ));

    let first_detail = service.get_book(first.id()).await.unwrap();
    assert_eq!(first_detail.book.status(), ReadingStatus::Reading);
    assert!(first_detail.notes.is_empty());
    let second_detail = service.get_book(second.id()).await.unwrap();
    assert_eq!(second_detail.book.status(), ReadingStatus::Finished);
    assert_eq!(second_detail.notes.len(), 1);
    assert_eq!(second_detail.notes[0].body.as_str(), "second note");
}

#[tokio::test]
async fn dropping_parent_future_stops_pending_completions_and_keeps_saved_results() {
    use std::time::Duration;

    let fake = FakeBookRepository::default();
    let service = ReadingService::new(fake.clone());
    let first = service
        .create_book(CreateBook {
            title: "First".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    let second = service
        .create_book(CreateBook {
            title: "Second".to_owned(),
            author: "Author".to_owned(),
        })
        .await
        .unwrap();
    for book_id in [first.id(), second.id()] {
        service
            .update_status(
                book_id,
                UpdateStatus {
                    status: "reading".to_owned(),
                },
            )
            .await
            .unwrap();
    }

    let control = fake.control_reading_completions([first.id(), second.id()]);
    tokio::time::timeout(Duration::from_secs(5), async {
        let run = service.record_reading_completions(RecordReadingCompletions {
            items: vec![
                RecordReadingCompletion {
                    book_id: first.id(),
                    body: "first note".to_owned(),
                },
                RecordReadingCompletion {
                    book_id: second.id(),
                    body: "second note".to_owned(),
                },
            ],
        });
        tokio::pin!(run);
        tokio::select! {
            _ = &mut run => panic!("the second completion is still waiting"),
            () = async {
                control.wait_for_started(2).await;
                control.release(first.id());
                control.wait_for_finished(1).await;
            } => {}
        }
    })
    .await
    .expect("the parent future should reach the controlled cancellation point");

    assert_eq!(control.finished(), vec![first.id()]);
    assert_eq!(control.dropped(), vec![second.id()]);

    let first_detail = service.get_book(first.id()).await.unwrap();
    assert_eq!(first_detail.book.status(), ReadingStatus::Finished);
    assert_eq!(first_detail.notes.len(), 1);
    assert_eq!(first_detail.notes[0].body.as_str(), "first note");
    let second_detail = service.get_book(second.id()).await.unwrap();
    assert_eq!(second_detail.book.status(), ReadingStatus::Reading);
    assert!(second_detail.notes.is_empty());
}

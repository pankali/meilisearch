use meili_snap::insta::assert_json_snapshot;
use serde_json::json;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::common::Server;

#[actix_rt::test]
async fn error_get_unexisting_task_status() {
    let server = Server::new().await;
    let index = server.index("test");
    index.create(None).await;
    index.wait_task(0).await;
    let (response, code) = index.get_task(1).await;

    let expected_response = json!({
        "message": "Task `1` not found.",
        "code": "task_not_found",
        "type": "invalid_request",
        "link": "https://docs.meilisearch.com/errors#task_not_found"
    });

    assert_eq!(response, expected_response);
    assert_eq!(code, 404);
}

#[actix_rt::test]
async fn get_task_status() {
    let server = Server::new().await;
    let index = server.index("test");
    index.create(None).await;
    index
        .add_documents(
            serde_json::json!([{
                "id": 1,
                "content": "foobar",
            }]),
            None,
        )
        .await;
    index.wait_task(0).await;
    let (_response, code) = index.get_task(1).await;
    assert_eq!(code, 200);
    // TODO check response format, as per #48
}

#[actix_rt::test]
async fn list_tasks() {
    let server = Server::new().await;
    let index = server.index("test");
    index.create(None).await;
    index.wait_task(0).await;
    index
        .add_documents(serde_json::from_str(include_str!("../assets/test_set.json")).unwrap(), None)
        .await;
    let (response, code) = index.list_tasks().await;
    assert_eq!(code, 200);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);
}

#[actix_rt::test]
async fn list_tasks_with_star_filters() {
    let server = Server::new().await;
    let index = server.index("test");
    index.create(None).await;
    index.wait_task(0).await;
    index
        .add_documents(serde_json::from_str(include_str!("../assets/test_set.json")).unwrap(), None)
        .await;
    let (response, code) = index.service.get("/tasks?indexUid=test").await;
    assert_eq!(code, 200);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);

    let (response, code) = index.service.get("/tasks?indexUid=*").await;
    assert_eq!(code, 200);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);

    let (response, code) = index.service.get("/tasks?indexUid=*,pasteque").await;
    assert_eq!(code, 200);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);

    let (response, code) = index.service.get("/tasks?type=*").await;
    assert_eq!(code, 200);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);

    let (response, code) =
        index.service.get("/tasks?type=*,documentAdditionOrUpdate&status=*").await;
    assert_eq!(code, 200, "{:?}", response);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);

    let (response, code) = index
        .service
        .get("/tasks?type=*,documentAdditionOrUpdate&status=*,failed&indexUid=test")
        .await;
    assert_eq!(code, 200, "{:?}", response);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);

    let (response, code) = index
        .service
        .get("/tasks?type=*,documentAdditionOrUpdate&status=*,failed&indexUid=test,*")
        .await;
    assert_eq!(code, 200, "{:?}", response);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);
}

#[actix_rt::test]
async fn list_tasks_status_filtered() {
    let server = Server::new().await;
    let index = server.index("test");
    index.create(None).await;
    index.wait_task(0).await;
    index
        .add_documents(serde_json::from_str(include_str!("../assets/test_set.json")).unwrap(), None)
        .await;

    let (response, code) = index.filtered_tasks(&[], &["succeeded"]).await;
    assert_eq!(code, 200, "{}", response);
    assert_eq!(response["results"].as_array().unwrap().len(), 1);

    // We can't be sure that the update isn't already processed so we can't test this
    // let (response, code) = index.filtered_tasks(&[], &["processing"]).await;
    // assert_eq!(code, 200, "{}", response);
    // assert_eq!(response["results"].as_array().unwrap().len(), 1);

    index.wait_task(1).await;

    let (response, code) = index.filtered_tasks(&[], &["succeeded"]).await;
    assert_eq!(code, 200, "{}", response);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);
}

#[actix_rt::test]
async fn list_tasks_type_filtered() {
    let server = Server::new().await;
    let index = server.index("test");
    index.create(None).await;
    index.wait_task(0).await;
    index
        .add_documents(serde_json::from_str(include_str!("../assets/test_set.json")).unwrap(), None)
        .await;

    let (response, code) = index.filtered_tasks(&["indexCreation"], &[]).await;
    assert_eq!(code, 200, "{}", response);
    assert_eq!(response["results"].as_array().unwrap().len(), 1);

    let (response, code) =
        index.filtered_tasks(&["indexCreation", "documentAdditionOrUpdate"], &[]).await;
    assert_eq!(code, 200, "{}", response);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);
}

#[actix_rt::test]
async fn list_tasks_status_and_type_filtered() {
    let server = Server::new().await;
    let index = server.index("test");
    index.create(None).await;
    index.wait_task(0).await;
    index
        .add_documents(serde_json::from_str(include_str!("../assets/test_set.json")).unwrap(), None)
        .await;

    let (response, code) = index.filtered_tasks(&["indexCreation"], &["failed"]).await;
    assert_eq!(code, 200, "{}", response);
    assert_eq!(response["results"].as_array().unwrap().len(), 0);

    let (response, code) = index
        .filtered_tasks(
            &["indexCreation", "documentAdditionOrUpdate"],
            &["succeeded", "processing", "enqueued"],
        )
        .await;
    assert_eq!(code, 200, "{}", response);
    assert_eq!(response["results"].as_array().unwrap().len(), 2);
}

macro_rules! assert_valid_summarized_task {
    ($response:expr, $task_type:literal, $index:literal) => {{
        assert_eq!($response.as_object().unwrap().len(), 5);
        assert!($response["taskUid"].as_u64().is_some());
        assert_eq!($response["indexUid"], $index);
        assert_eq!($response["status"], "enqueued");
        assert_eq!($response["type"], $task_type);
        let date = $response["enqueuedAt"].as_str().expect("missing date");

        OffsetDateTime::parse(date, &Rfc3339).unwrap();
    }};
}

#[actix_web::test]
async fn test_summarized_task_view() {
    let server = Server::new().await;
    let index = server.index("test");

    let (response, _) = index.create(None).await;
    assert_valid_summarized_task!(response, "indexCreation", "test");

    let (response, _) = index.update(None).await;
    assert_valid_summarized_task!(response, "indexUpdate", "test");

    let (response, _) = index.update_settings(json!({})).await;
    assert_valid_summarized_task!(response, "settingsUpdate", "test");

    let (response, _) = index.update_documents(json!([{"id": 1}]), None).await;
    assert_valid_summarized_task!(response, "documentAdditionOrUpdate", "test");

    let (response, _) = index.add_documents(json!([{"id": 1}]), None).await;
    assert_valid_summarized_task!(response, "documentAdditionOrUpdate", "test");

    let (response, _) = index.delete_document(1).await;
    assert_valid_summarized_task!(response, "documentDeletion", "test");

    let (response, _) = index.clear_all_documents().await;
    assert_valid_summarized_task!(response, "documentDeletion", "test");

    let (response, _) = index.delete().await;
    assert_valid_summarized_task!(response, "indexDeletion", "test");
}

#[actix_web::test]
async fn test_summarized_document_addition_or_update() {
    let server = Server::new().await;
    let index = server.index("test");
    index.add_documents(json!({ "id": 42, "content": "doggos & fluff" }), None).await;
    index.wait_task(0).await;
    let (task, _) = index.get_task(0).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 0,
      "indexUid": "test",
      "status": "succeeded",
      "type": "documentAdditionOrUpdate",
      "details": {
        "receivedDocuments": 1,
        "indexedDocuments": 1
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    index.add_documents(json!({ "id": 42, "content": "doggos & fluff" }), Some("id")).await;
    index.wait_task(1).await;
    let (task, _) = index.get_task(1).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 1,
      "indexUid": "test",
      "status": "succeeded",
      "type": "documentAdditionOrUpdate",
      "details": {
        "receivedDocuments": 1,
        "indexedDocuments": 1
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_delete_batch() {
    let server = Server::new().await;
    let index = server.index("test");
    index.delete_batch(vec![1, 2, 3]).await;
    index.wait_task(0).await;
    let (task, _) = index.get_task(0).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 0,
      "indexUid": "test",
      "status": "failed",
      "type": "documentDeletion",
      "details": {
        "matchedDocuments": 3,
        "deletedDocuments": null
      },
      "error": {
        "message": "Index `test` not found.",
        "code": "index_not_found",
        "type": "invalid_request",
        "link": "https://docs.meilisearch.com/errors#index_not_found"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    index.create(None).await;
    index.delete_batch(vec![42]).await;
    index.wait_task(2).await;
    let (task, _) = index.get_task(2).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 2,
      "indexUid": "test",
      "status": "succeeded",
      "type": "documentDeletion",
      "details": {
        "matchedDocuments": 1,
        "deletedDocuments": 0
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_delete_document() {
    let server = Server::new().await;
    let index = server.index("test");
    index.delete_document(1).await;
    index.wait_task(0).await;
    let (task, _) = index.get_task(0).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 0,
      "indexUid": "test",
      "status": "failed",
      "type": "documentDeletion",
      "details": {
        "matchedDocuments": 1,
        "deletedDocuments": null
      },
      "error": {
        "message": "Index `test` not found.",
        "code": "index_not_found",
        "type": "invalid_request",
        "link": "https://docs.meilisearch.com/errors#index_not_found"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    index.create(None).await;
    index.delete_document(42).await;
    index.wait_task(2).await;
    let (task, _) = index.get_task(2).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 2,
      "indexUid": "test",
      "status": "succeeded",
      "type": "documentDeletion",
      "details": {
        "matchedDocuments": 1,
        "deletedDocuments": 0
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_settings_update() {
    let server = Server::new().await;
    let index = server.index("test");
    // here we should find my payload even in the failed task.
    index.update_settings(json!({ "rankingRules": ["custom"] })).await;
    index.wait_task(0).await;
    let (task, _) = index.get_task(0).await;
    dbg!(&task);
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 0,
      "indexUid": "test",
      "status": "failed",
      "type": "settingsUpdate",
      "details": {
        "rankingRules": [
          "custom"
        ]
      },
      "error": {
        "message": "`custom` ranking rule is invalid. Valid ranking rules are words, typo, sort, proximity, attribute, exactness and custom ranking rules.",
        "code": "invalid_ranking_rule",
        "type": "invalid_request",
        "link": "https://docs.meilisearch.com/errors#invalid_ranking_rule"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    index.update_settings(json!({ "displayedAttributes": ["doggos", "name"], "filterableAttributes": ["age", "nb_paw_pads"], "sortableAttributes": ["iq"] })).await;
    index.wait_task(1).await;
    let (task, _) = index.get_task(1).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 1,
      "indexUid": "test",
      "status": "succeeded",
      "type": "settingsUpdate",
      "details": {
        "displayedAttributes": [
          "doggos",
          "name"
        ],
        "filterableAttributes": [
          "age",
          "nb_paw_pads"
        ],
        "sortableAttributes": [
          "iq"
        ]
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_index_creation() {
    let server = Server::new().await;
    let index = server.index("test");
    index.create(None).await;
    index.wait_task(0).await;
    let (task, _) = index.get_task(0).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 0,
      "indexUid": "test",
      "status": "succeeded",
      "type": "indexCreation",
      "details": {
        "primaryKey": null
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    index.create(Some("doggos")).await;
    index.wait_task(1).await;
    let (task, _) = index.get_task(1).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 1,
      "indexUid": "test",
      "status": "failed",
      "type": "indexCreation",
      "details": {
        "primaryKey": "doggos"
      },
      "error": {
        "message": "Index `test` already exists.",
        "code": "index_already_exists",
        "type": "invalid_request",
        "link": "https://docs.meilisearch.com/errors#index_already_exists"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_index_deletion() {
    let server = Server::new().await;
    let index = server.index("test");
    index.delete().await;
    index.wait_task(0).await;
    let (task, _) = index.get_task(0).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 0,
      "indexUid": "test",
      "status": "failed",
      "type": "indexDeletion",
      "error": {
        "message": "Index `test` not found.",
        "code": "index_not_found",
        "type": "invalid_request",
        "link": "https://docs.meilisearch.com/errors#index_not_found"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    // is the details correctly set when documents are actually deleted.
    index.add_documents(json!({ "id": 42, "content": "doggos & fluff" }), Some("id")).await;
    index.delete().await;
    index.wait_task(2).await;
    let (task, _) = index.get_task(2).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 2,
      "indexUid": "test",
      "status": "succeeded",
      "type": "indexDeletion",
      "details": {
        "deletedDocuments": 1
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    // What happens when you delete an index that doesn't exists.
    index.delete().await;
    index.wait_task(2).await;
    let (task, _) = index.get_task(2).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 2,
      "indexUid": "test",
      "status": "succeeded",
      "type": "indexDeletion",
      "details": {
        "deletedDocuments": 1
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_index_update() {
    let server = Server::new().await;
    let index = server.index("test");
    // If the index doesn't exist yet, we should get errors with or without the primary key.
    index.update(None).await;
    index.wait_task(0).await;
    let (task, _) = index.get_task(0).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 0,
      "indexUid": "test",
      "status": "failed",
      "type": "indexUpdate",
      "details": {
        "primaryKey": null
      },
      "error": {
        "message": "Index `test` not found.",
        "code": "index_not_found",
        "type": "invalid_request",
        "link": "https://docs.meilisearch.com/errors#index_not_found"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    index.update(Some("bones")).await;
    index.wait_task(1).await;
    let (task, _) = index.get_task(1).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 1,
      "indexUid": "test",
      "status": "failed",
      "type": "indexUpdate",
      "details": {
        "primaryKey": "bones"
      },
      "error": {
        "message": "Index `test` not found.",
        "code": "index_not_found",
        "type": "invalid_request",
        "link": "https://docs.meilisearch.com/errors#index_not_found"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    // And run the same two tests once the index do exists.
    index.create(None).await;

    index.update(None).await;
    index.wait_task(3).await;
    let (task, _) = index.get_task(3).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 3,
      "indexUid": "test",
      "status": "succeeded",
      "type": "indexUpdate",
      "details": {
        "primaryKey": null
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    index.update(Some("bones")).await;
    index.wait_task(4).await;
    let (task, _) = index.get_task(4).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 4,
      "indexUid": "test",
      "status": "succeeded",
      "type": "indexUpdate",
      "details": {
        "primaryKey": "bones"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_index_swap() {
    let server = Server::new().await;
    server
        .index_swap(json!([
            { "indexes": ["doggos", "cattos"] }
        ]))
        .await;
    server.wait_task(0).await;
    let (task, _) = server.get_task(0).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 0,
      "indexUid": null,
      "status": "failed",
      "type": "indexSwap",
      "details": {
        "swaps": [
          {
            "indexes": [
              "doggos",
              "cattos"
            ]
          }
        ]
      },
      "error": {
        "message": "Indexes `cattos`, `doggos` not found.",
        "code": "index_not_found",
        "type": "invalid_request",
        "link": "https://docs.meilisearch.com/errors#index_not_found"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);

    server.index("doggos").create(None).await;
    server.index("cattos").create(None).await;
    server
        .index_swap(json!([
            { "indexes": ["doggos", "cattos"] }
        ]))
        .await;
    server.wait_task(3).await;
    let (task, _) = server.get_task(3).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 3,
      "indexUid": null,
      "status": "succeeded",
      "type": "indexSwap",
      "details": {
        "swaps": [
          {
            "indexes": [
              "doggos",
              "cattos"
            ]
          }
        ]
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_task_cancelation() {
    let server = Server::new().await;
    let index = server.index("doggos");
    // to avoid being flaky we're only going to cancel an already finished task :(
    index.create(None).await;
    index.wait_task(0).await;
    server.cancel_task(json!({ "uid": [0] })).await;
    index.wait_task(1).await;
    let (task, _) = index.get_task(1).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 1,
      "indexUid": null,
      "status": "succeeded",
      "type": "taskCancelation",
      "details": {
        "matchedTasks": 1,
        "canceledTasks": 0,
        "originalQuery": "uid=0"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_task_deletion() {
    let server = Server::new().await;
    let index = server.index("doggos");
    // to avoid being flaky we're only going to delete an already finished task :(
    index.create(None).await;
    index.wait_task(0).await;
    server.delete_task(json!({ "uid": [0] })).await;
    index.wait_task(1).await;
    let (task, _) = index.get_task(1).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 1,
      "indexUid": null,
      "status": "succeeded",
      "type": "taskDeletion",
      "details": {
        "matchedTasks": 1,
        "deletedTasks": 1,
        "originalQuery": "uid=0"
      },
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

#[actix_web::test]
async fn test_summarized_dump_creation() {
    let server = Server::new().await;
    server.create_dump().await;
    server.wait_task(0).await;
    let (task, _) = server.get_task(0).await;
    assert_json_snapshot!(task, 
        { ".duration" => "[duration]", ".enqueuedAt" => "[date]", ".startedAt" => "[date]", ".finishedAt" => "[date]" },
        @r###"
    {
      "uid": 0,
      "indexUid": null,
      "status": "succeeded",
      "type": "dumpCreation",
      "duration": "[duration]",
      "enqueuedAt": "[date]",
      "startedAt": "[date]",
      "finishedAt": "[date]"
    }
    "###);
}

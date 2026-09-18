use typesafe_sdk::{Client, Noul};

#[tokio::test]
#[ignore = "requires TYPESAFE_API_KEY and network access"]
async fn live_models_and_system_one() {
    let client = Client::from_env().unwrap();
    assert!(!client.models().await.unwrap().models.is_empty());
    let response = client
        .system_one("I was charged twice.")
        .question("billing", Noul::new("Is this about billing?"))
        .send()
        .await
        .unwrap();
    assert!(response.noul("billing").is_some());
}

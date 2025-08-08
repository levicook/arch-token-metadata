use std::time::Duration;

use arch_testing::TestRunner;
use tokio::time::sleep;

#[tokio::test]
async fn simple_test() {
    TestRunner::run(|_ctx| async move {
        println!("Test function started");
        sleep(Duration::from_millis(100)).await;
        println!("Test function completed");
        Ok(())
    })
    .await
}

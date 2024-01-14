use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use url::Url;

pub struct LogControl {
    task: tokio::task::JoinHandle<()>,
    controller: tracing_loki::BackgroundTaskController,
}

impl LogControl {
    pub fn init_logging() -> Self {
        let sqlx_filter = EnvFilter::new(
            "\
            info,\
            sqlx::query=warn,\
            hyper=warn\
        ",
        );

        // let sqlx_layer = tracing_subscriber::fmt::layer().with_filter(sqlx_filter);

        // getting
        let (layer, controller, task) = tracing_loki::builder()
            // used to set a label on the logs
            .label("source", "mare-website")
            .unwrap()
            // additional key-value pairs that provide more context or information about the log event
            .extra_field("pid", format!("{}", std::process::id()))
            .unwrap()
            .build_controller_url(Url::parse("http://loki:3100").unwrap())
            .unwrap();

        // register our layer with `tracing`.
        tracing_subscriber::registry()
            .with(layer)
            // .with(sqlx_layer)
            // One could add more layers here, for example logging to stdout:
            .with(tracing_subscriber::fmt::Layer::new())
            .with(sqlx_filter)
            .init();

        // The background task needs to be spawned so the logs actually get
        // delivered.
        let task = tokio::spawn(task);

        info!("Logging successfully set up",);

        Self { task, controller }
    }

    pub async fn shutdown(self) {
        info!("Shutting down logging task");

        self.controller.shutdown().await;

        eprintln!("Stopped logging task: {:?}", self.task.await);
    }
}

use crate::Wrapper;

mod quote;

pub async fn handle_quote_request(
    report_data: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let report_data = hex::decode(report_data).map_err(|_| Wrapper("invalid hex".into()))?;
    let quote = quote::get_quote(&report_data).await;

    match quote {
        Ok(quote) => {
            tracing::info!("successfully obtained quote");
            Ok(warp::reply::with_status(quote, warp::http::StatusCode::OK))
        }

        Err(e) => {
            tracing::error!("failed to obtain quote: {:?}", e);
            Ok(warp::reply::with_status(
                "failed to get quote".into(),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

use crate::{
    core::{object_id::ObjectId, signatures::verify_signature},
    data::Data,
    traits::{ActivityHandler, Actor, ApubObject},
    utils::{verify_domains_match, verify_url_valid},
    Error,
    LocalInstance,
};
use actix_web::{
    web::{Bytes, Payload},
    FromRequest,
    HttpRequest,
    HttpResponse,
};
use anyhow::anyhow;
use http_signature_normalization_actix::prelude::DigestVerified;
use serde::de::DeserializeOwned;
use tracing::{log::debug, warn};

/// Receive an activity and perform some basic checks, including HTTP signature verification.
pub async fn receive_activity<Activity, ActorT, Datatype>(
    request: HttpRequest,
    payload: Payload,
    local_instance: &LocalInstance,
    data: &Data<Datatype>,
) -> Result<HttpResponse, <Activity as ActivityHandler>::Error>
where
    Activity: ActivityHandler<DataType = Datatype> + DeserializeOwned + Send + 'static,
    ActorT: ApubObject<DataType = Datatype> + Actor + Send + 'static,
    for<'de2> <ActorT as ApubObject>::ApubType: serde::Deserialize<'de2>,
    <Activity as ActivityHandler>::Error: From<anyhow::Error>
        + From<Error>
        + From<<ActorT as ApubObject>::Error>
        + From<serde_json::Error>
        + From<http_signature_normalization_actix::digest::middleware::VerifyError>,
    <ActorT as ApubObject>::Error: From<Error> + From<anyhow::Error>,
{
    let mut payload = payload.into_inner();
    // ensure that payload hash was checked against digest header by middleware
    DigestVerified::from_request(&request, &mut payload).await?;

    let bytes = Bytes::from_request(&request, &mut payload)
        .await
        .map_err(|e| {
            warn!("{}", e);
            anyhow!("Failed to parse request body")
        })?;
    let activity: Activity = serde_json::from_slice(&bytes)?;

    verify_domains_match(activity.id(), activity.actor())?;
    verify_url_valid(activity.id(), &local_instance.settings).await?;
    if local_instance.is_local_url(activity.id()) {
        return Err(Error::UrlVerificationError("Activity was sent from local instance").into());
    }

    let request_counter = &mut 0;
    let actor = ObjectId::<ActorT>::new(activity.actor().clone())
        .dereference(data, local_instance, request_counter)
        .await?;
    verify_signature(&request, actor.public_key())?;

    debug!("Verifying activity {}", activity.id().to_string());
    activity.verify(data, request_counter).await?;

    debug!("Receiving activity {}", activity.id().to_string());
    activity.receive(data, request_counter).await?;
    Ok(HttpResponse::Ok().finish())
}

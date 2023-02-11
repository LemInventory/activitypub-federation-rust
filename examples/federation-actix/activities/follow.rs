use crate::{
    activities::accept::Accept,
    generate_object_id,
    instance::DatabaseHandle,
    objects::person::MyUser,
};
use activitypub_federation::{
    core::object_id::ObjectId,
    request_data::RequestData,
    traits::{ActivityHandler, Actor},
};
use activitystreams_kinds::activity::FollowType;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Follow {
    pub(crate) actor: ObjectId<MyUser>,
    pub(crate) object: ObjectId<MyUser>,
    #[serde(rename = "type")]
    kind: FollowType,
    id: Url,
}

impl Follow {
    pub fn new(actor: ObjectId<MyUser>, object: ObjectId<MyUser>, id: Url) -> Follow {
        Follow {
            actor,
            object,
            kind: Default::default(),
            id,
        }
    }
}

#[async_trait::async_trait]
impl ActivityHandler for Follow {
    type DataType = DatabaseHandle;
    type Error = crate::error::Error;

    fn id(&self) -> &Url {
        &self.id
    }

    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    // Ignore clippy false positive: https://github.com/rust-lang/rust-clippy/issues/6446
    #[allow(clippy::await_holding_lock)]
    async fn receive(self, data: &RequestData<Self::DataType>) -> Result<(), Self::Error> {
        // add to followers
        let local_user = {
            let mut users = data.users.lock().unwrap();
            let local_user = users.first_mut().unwrap();
            local_user.followers.push(self.actor.inner().clone());
            local_user.clone()
        };

        // send back an accept
        let follower = self.actor.dereference(data).await?;
        let id = generate_object_id(data.local_instance().hostname())?;
        let accept = Accept::new(local_user.ap_id.clone(), self, id.clone());
        local_user
            .send(accept, vec![follower.shared_inbox_or_inbox()], data)
            .await?;
        Ok(())
    }
}

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use kube::api::{Api, DynamicObject, ListParams};
use kube::{discovery, Client};

use futures::{StreamExt, TryStreamExt};
use kube_runtime::{utils::try_flatten_applied, watcher};

use anyhow::{bail, Result};

use crate::api::ApiDefinition;
use crate::route::Node;
use kube::core::GroupVersionKind;

pub async fn update_api(
    api_lock: Arc<RwLock<HashMap<String, (ApiDefinition, Node)>>>,
    label_filter: String,
) -> Result<()> {
    let client = match Client::try_default().await {
        Ok(client) => client,
        Err(e) => {
            let err_msg = format!("kube client: {:?}", e);
            error!("event='{}'", err_msg);
            bail!(err_msg);
        }
    };
    let group = "gateway.dgexsol.fr";
    let version = "v1";
    let kind = "ApiDefinition";

    let gvk = GroupVersionKind::gvk(group, version, kind);
    // Use API discovery to identify more information about the type (like its plural)
    let (ar, _caps) = discovery::pinned_kind(&client, &gvk).await?;

    // Use the discovered kind in an Api with the ApiResource as its DynamicType
    let apidefinitions = Api::<DynamicObject>::all_with(client, &ar);
    let lp = ListParams::default().labels(&label_filter);
    let watcher = watcher(apidefinitions, lp);

    let mut apply_apidefinitions = try_flatten_applied(watcher).boxed_local();
    loop {
        match apply_apidefinitions.try_next().await {
            Err(e) => {
                let err_msg = format!("Crd stream: {:?}", e);
                error!("event='{}'", err_msg);
                bail!(err_msg);
            }
            Ok(None) => {
                info!("event='No apidefinition found'");
            }
            Ok(Some(ref apidefinition)) => match ApiDefinition::try_from(apidefinition) {
                Err(e) => {
                    let err_msg = format!(
                        "event='An error occurs during apidefinition parsing: {}'",
                        e
                    );
                    error!("event='{}'", err_msg);
                    bail!(err_msg);
                }
                Ok(apidefinition) => match apidefinition.check_fields() {
                    Err(e) => {
                        let err_msg = format!("Invalid apidefinition: {}", e);
                        error!("event='{}'", err_msg);
                        bail!(err_msg);
                    }
                    Ok(_) => {
                        let node = Node::new(&apidefinition);
                        let mut api_write = api_lock.write().await;
                        let mut built_apidefinition = apidefinition.clone();
                        built_apidefinition.build_uri();
                        api_write.insert(
                            built_apidefinition.spec.app_name.clone(),
                            (built_apidefinition, node),
                        );
                        info!(
                            "event='{} api updated from {:?}'",
                            &apidefinition.spec.app_name,
                            &apidefinition
                                .metadata
                                .name
                                .as_ref()
                                .unwrap_or(&"NO_NAME_DEFINED".to_owned())
                        );
                    }
                },
            },
        };
    }
}

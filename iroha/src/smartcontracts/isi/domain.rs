//! This module contains `Domain` structure and related implementations and trait implementations.
use std::collections::btree_map::Entry;

use iroha_data_model::prelude::*;
use iroha_error::{error, Result};

use super::super::isi::prelude::*;
use crate::prelude::*;

/// Iroha Special Instructions module provides `DomainInstruction` enum with all possible types of
/// Domain related instructions as variants, implementations of generic Iroha Special Instructions
/// and the `From/Into` implementations to convert `DomainInstruction` variants into generic ISI.
pub mod isi {
    use super::*;

    impl Execute for Register<NewAccount> {
        fn execute(
            self,
            _authority: <NewAccount as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Error> {
            let account = self.object;
            account.validate_len(wsv.config.length_limits)?;
            let name = account.id.domain_name.clone();
            match wsv.domain_mut(&name)?.accounts.entry(account.id.clone()) {
                Entry::Occupied(_) => {
                    return Err(error!(
                        "Domain already contains an account with this Id: {:?}",
                        &account.id
                    )
                    .into())
                }
                Entry::Vacant(entry) => {
                    let _ = entry.insert(account.into());
                }
            }
            Ok(())
        }
    }

    impl Execute for Unregister<Account> {
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Error> {
            let account_id = self.object_id;
            drop(
                wsv.domain_mut(&account_id.domain_name)?
                    .accounts
                    .remove(&account_id),
            );
            Ok(())
        }
    }

    impl Execute for Register<AssetDefinition> {
        fn execute(
            self,
            authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Error> {
            let asset_definition = self.object;
            asset_definition.validate_len(wsv.config.length_limits)?;
            let name = asset_definition.id.domain_name.clone();
            let mut domain = wsv.domain_mut(&name)?;
            match domain.asset_definitions.entry(asset_definition.id.clone()) {
                Entry::Vacant(entry) => {
                    let _ = entry.insert(AssetDefinitionEntry {
                        definition: asset_definition,
                        registered_by: authority,
                    });
                }
                Entry::Occupied(entry) => {
                    return Err(error!(
                        "Asset definition already exists and was registered by {}",
                        entry.get().registered_by
                    )
                    .into())
                }
            }
            Ok(())
        }
    }

    impl Execute for Unregister<AssetDefinition> {
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Error> {
            let asset_definition_id = self.object_id;
            drop(
                wsv.domain_mut(&asset_definition_id.domain_name)?
                    .asset_definitions
                    .remove(&asset_definition_id),
            );
            for mut domain in wsv.domains().iter_mut() {
                for account in domain.accounts.values_mut() {
                    let keys = account
                        .assets
                        .iter()
                        .filter(|(asset_id, _asset)| asset_id.definition_id == asset_definition_id)
                        .map(|(asset_id, _asset)| asset_id.clone())
                        .collect::<Vec<_>>();
                    keys.iter().for_each(|asset_id| {
                        drop(account.assets.remove(asset_id));
                    });
                }
            }
            Ok(())
        }
    }
}

/// Query module provides `IrohaQuery` Domain related implementations.
pub mod query {
    use iroha_error::{Result, WrapErr};
    use iroha_logger::log;

    use super::super::expression::Evaluate;
    use super::*;

    impl Query for FindAllDomains {
        #[log]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output> {
            Ok(wsv
                .domains()
                .iter()
                .map(|guard| guard.value().clone())
                .collect())
        }
    }

    impl Query for FindDomainByName {
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output> {
            let name = self
                .name
                .evaluate(wsv, &Context::default())
                .wrap_err("Failed to get domain name")?;
            Ok(wsv.domain(&name)?.clone())
        }
    }
}

use std::collections::HashMap;

use crate::metadata::{
    ConversionError, Metadata, ModuleConstantMetadata, ModuleMetadata, ModuleWithCalls,
    ModuleWithConstants, ModuleWithErrors, ModuleWithEvents, StorageMetadata,
};
use metadata::RuntimeMetadataV14;
use scale_info::TypeDef;

pub(crate) fn parse_metadata_v14(meta: RuntimeMetadataV14) -> Result<Metadata, ConversionError> {
    let mut modules = HashMap::new();
    let mut modules_with_calls = HashMap::new();
    let mut modules_with_events = HashMap::new();
    let mut modules_with_errors = HashMap::new();
    let mut modules_with_constants = HashMap::new();

    let type_registry = meta.types.types();

    for module in meta.pallets {
        let module_name = module.name;

        let mut storage_map = HashMap::new();
        if let Some(storage) = module.storage {
            let module_prefix = storage.prefix;
            for entry in storage.entries.into_iter() {
                let storage_prefix = entry.name.clone();
                let entry = convert_entry(module_prefix.clone(), storage_prefix.clone(), entry)?;
                storage_map.insert(storage_prefix, entry);
            }
        }
        modules.insert(
            module_name.clone(),
            ModuleMetadata {
                index: module.index,
                name: module_name.clone(),
                storage: storage_map,
            },
        );

        if let Some(calls) = module.calls {
            let mut call_map = HashMap::new();
            match type_registry[calls.ty.id() as usize].ty().type_def() {
                TypeDef::Variant(v) => {
                    for (index, call) in v.variants().iter().enumerate() {
                        call_map.insert(call.name().clone(), index as u8);
                    }
                }
                _ => {
                    log::warn!(
                        "Skipped parsing calls for module {}, type ({}) is not variant",
                        module_name,
                        calls.ty.id()
                    );
                }
            };

            modules_with_calls.insert(
                module_name.clone(),
                ModuleWithCalls {
                    index: module.index,
                    name: module_name.clone(),
                    calls: call_map,
                },
            );
        }

        if let Some(events) = module.event {
            let mut event_map = HashMap::new();
            match type_registry[events.ty.id() as usize].ty().type_def() {
                TypeDef::Variant(v) => {
                    for (index, call) in v.variants().iter().enumerate() {
                        event_map.insert(index as u8, call.name().clone());
                    }
                }
                _ => {
                    log::warn!(
                        "Skipped parsing events for module {}, type ({}) is not variant",
                        module_name,
                        events.ty.id()
                    );
                }
            };

            modules_with_events.insert(
                module_name.clone(),
                ModuleWithEvents {
                    index: module.index,
                    name: module_name.clone(),
                    events: event_map,
                },
            );
        }

        if let Some(errors) = module.error {
            let mut error_map = HashMap::new();

            match type_registry[errors.ty.id() as usize].ty().type_def() {
                TypeDef::Variant(v) => {
                    for (index, err) in v.variants().iter().enumerate() {
                        error_map.insert(index as u8, err.name().clone());
                    }
                }
                _ => {
                    log::warn!(
                        "Skipped parsing errors for module {}, type ({}) is not variant",
                        module_name,
                        errors.ty.id()
                    );
                }
            };

            modules_with_errors.insert(
                module_name.clone(),
                ModuleWithErrors {
                    index: module.index,
                    name: module_name.clone(),
                    errors: error_map,
                },
            );
        }

        let constants = module.constants;
        let mut constant_map = HashMap::new();
        for (index, constant) in constants.into_iter().enumerate() {
            constant_map.insert(
                index as u8,
                ModuleConstantMetadata {
                    ty: type_registry[constant.ty.id() as usize]
                        .ty()
                        .path()
                        .to_string(),
                    name: constant.name,
                    value: constant.value,
                },
            );
        }
        modules_with_constants.insert(
            module_name.clone(),
            ModuleWithConstants {
                index: module.index,
                name: module_name.clone(),
                constants: constant_map,
            },
        );
    }
    Ok(Metadata {
        modules,
        modules_with_calls,
        modules_with_events,
        modules_with_errors,
        modules_with_constants,
    })
}

fn convert_entry(
    module_prefix: String,
    storage_prefix: String,
    entry: metadata::StorageEntryMetadata<scale_info::form::PortableForm>,
) -> Result<StorageMetadata, ConversionError> {
    let default = entry.default;
    Ok(StorageMetadata {
        module_prefix,
        storage_prefix,
        modifier: entry.modifier,
        ty: Some(entry.ty),
        default,
    })
}

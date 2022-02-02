use std::collections::HashMap;

use crate::metadata::{
    ConversionError, Metadata, ModuleConstantMetadata, ModuleMetadata, ModuleWithCalls,
    ModuleWithConstants, ModuleWithErrors, ModuleWithEvents, StorageMetadata,
};
use metadata::decode_different::DecodeDifferent;
use metadata::v13::{StorageEntryModifier, StorageEntryType, StorageHasher};

pub(crate) fn parse_metadata_v13(
    meta: metadata::v13::RuntimeMetadataV13,
) -> Result<Metadata, ConversionError> {
    let mut modules: HashMap<String, ModuleMetadata> = HashMap::new();
    let mut modules_with_calls: HashMap<String, ModuleWithCalls> = HashMap::new();
    let mut modules_with_errors: HashMap<String, ModuleWithErrors> = HashMap::new();
    let mut modules_with_events: HashMap<String, ModuleWithEvents> = HashMap::new();
    let mut modules_with_constants: HashMap<String, ModuleWithConstants> = HashMap::new();

    for module in convert(meta.modules)?.into_iter() {
        let module_name = convert(module.name.clone())?;

        let mut storage_map = HashMap::new();
        if let Some(storage) = module.storage {
            let storage = convert(storage)?;
            let module_prefix = convert(storage.prefix)?;
            for entry in convert(storage.entries)?.into_iter() {
                let storage_prefix = convert(entry.name.clone())?;
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
            for (index, call) in convert(calls)?.into_iter().enumerate() {
                let name = convert(call.name)?;
                call_map.insert(name, index as u8);
            }
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
            for (index, event) in convert(events)?.into_iter().enumerate() {
                event_map.insert(index as u8, convert_event(event)?);
            }
            modules_with_events.insert(
                module_name.clone(),
                ModuleWithEvents {
                    index: module.index,
                    name: module_name.clone(),
                    events: event_map,
                },
            );
        }
        let errors = module.errors;
        let mut error_map = HashMap::new();
        for (index, error) in convert(errors)?.into_iter().enumerate() {
            let name = convert(error.name)?;
            error_map.insert(index as u8, name);
        }
        modules_with_errors.insert(
            module_name.clone(),
            ModuleWithErrors {
                index: module.index,
                name: module_name.clone(),
                errors: error_map,
            },
        );
        let constants = module.constants;
        let mut constant_map = HashMap::new();
        for (index, constant) in convert(constants)?.into_iter().enumerate() {
            constant_map.insert(index as u8, convert_constant(constant)?);
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

fn convert<B: 'static, O: 'static>(dd: DecodeDifferent<B, O>) -> Result<O, ConversionError> {
    match dd {
        DecodeDifferent::Decoded(value) => Ok(value),
        _ => Err(ConversionError::ExpectedDecoded),
    }
}

fn convert_event(event: metadata::v13::EventMetadata) -> Result<String, ConversionError> {
    convert(event.name).map(|s| s.to_string())
}

fn convert_constant(
    constant: metadata::v13::ModuleConstantMetadata,
) -> Result<ModuleConstantMetadata, ConversionError> {
    let name = convert(constant.name)?;
    let value = convert(constant.value)?;
    let ty = convert(constant.ty)?;
    Ok(ModuleConstantMetadata { name, ty, value })
}

fn convert_entry(
    module_prefix: String,
    storage_prefix: String,
    entry: metadata::v13::StorageEntryMetadata,
) -> Result<StorageMetadata, ConversionError> {
    let default = convert(entry.default)?;
    let ty = match entry.ty {
        StorageEntryType::Plain(_) => crate::metadata::StorageEntryType::Plain,
        StorageEntryType::Map { hasher, .. } => crate::metadata::StorageEntryType::Map {
            hashers: vec![convert_hasher(&hasher)],
        },
        StorageEntryType::DoubleMap { hasher, key2_hasher, .. } => crate::metadata::StorageEntryType::Map {
            hashers: vec![convert_hasher(&hasher), convert_hasher(&key2_hasher)],
        },
        StorageEntryType::NMap { hashers, .. } => {
            let hashers: Vec<metadata::v14::StorageHasher> = match hashers {
                DecodeDifferent::Encode(b) => b.iter().map(|x| convert_hasher(x)).collect(),
                DecodeDifferent::Decoded(o) => o.iter().map(|x| convert_hasher(x)).collect(),
            };
            crate::metadata::StorageEntryType::Map {
                hashers
            }
        }
    };
    Ok(StorageMetadata {
        module_prefix,
        storage_prefix,
        modifier: match entry.modifier {
            StorageEntryModifier::Optional => metadata::v14::StorageEntryModifier::Optional,
            StorageEntryModifier::Default => metadata::v14::StorageEntryModifier::Default,
        },
        ty,
        default,
    })
}

fn convert_hasher(hasher: &metadata::v13::StorageHasher) -> metadata::v14::StorageHasher {
    match hasher {
        StorageHasher::Blake2_128 => metadata::v14::StorageHasher::Blake2_128,
        StorageHasher::Blake2_256 => metadata::v14::StorageHasher::Blake2_256,
        StorageHasher::Blake2_128Concat => metadata::v14::StorageHasher::Blake2_128Concat,
        StorageHasher::Twox128 => metadata::v14::StorageHasher::Twox128,
        StorageHasher::Twox256 => metadata::v14::StorageHasher::Twox256,
        StorageHasher::Twox64Concat => metadata::v14::StorageHasher::Twox64Concat,
        StorageHasher::Identity => metadata::v14::StorageHasher::Identity,
    }
}

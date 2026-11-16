use crate::ab_host::{
    AppInfo, DeviceInfo, DialogResult, PickResult, RenderSize, SaveSession, WatchfaceInfo,
};
use crate::component::ComponentInstanceContext;
use crate::runtime_error::{bail, Result};

use serde_json::{json, Value};
use wasmtime::component::types::ComponentItem;
use wasmtime::component::{Component, FutureReader, Linker, LinkerInstance, Type, Val};

use wasmtime::error::Context;

pub fn val_to_json(value: &Val) -> Result<Value> {
    Ok(match value {
        Val::Bool(v) => json!(v),
        Val::U8(v) => json!(v),
        Val::U16(v) => json!(v),
        Val::U32(v) => json!(v),
        Val::U64(v) => json!(v),
        Val::S8(v) => json!(v),
        Val::S16(v) => json!(v),
        Val::S32(v) => json!(v),
        Val::S64(v) => json!(v),
        Val::Float32(v) => json!(v),
        Val::Float64(v) => json!(v),
        Val::String(v) | Val::Enum(v) => json!(v),
        Val::Char(v) => json!(v.to_string()),
        Val::List(values) | Val::Tuple(values) => {
            Value::Array(values.iter().map(val_to_json).collect::<Result<_>>()?)
        }
        Val::Record(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| Ok((key.clone(), val_to_json(value)?)))
                .collect::<Result<_>>()?,
        ),
        Val::Option(value) => match value {
            Some(value) => val_to_json(value)?,
            None => Value::Null,
        },
        Val::Variant(name, value) => {
            json!({"case": name, "value": value.as_ref().map(|v| val_to_json(v)).transpose()?})
        }
        Val::Result(value) => match value {
            Ok(value) => json!({"Ok": value.as_ref().map(|v| val_to_json(v)).transpose()?}),
            Err(value) => json!({"Err": value.as_ref().map(|v| val_to_json(v)).transpose()?}),
        },
        _ => bail!("Unsupported host argument {value:?}"),
    })
}

pub fn json_to_val(ty: &Type, value: &Value) -> Result<Val> {
    Ok(match ty {
        Type::Bool => Val::Bool(value.as_bool().context("Expected bool")?),
        Type::U8 => Val::U8(value.as_u64().context("Expected u8")?.try_into()?),
        Type::U16 => Val::U16(value.as_u64().context("Expected u16")?.try_into()?),
        Type::U32 => Val::U32(value.as_u64().context("Expected u32")?.try_into()?),
        Type::U64 => Val::U64(value.as_u64().context("Expected u64")?),
        Type::S8 => Val::S8(value.as_i64().context("Expected s8")?.try_into()?),
        Type::S16 => Val::S16(value.as_i64().context("Expected s16")?.try_into()?),
        Type::S32 => Val::S32(value.as_i64().context("Expected s32")?.try_into()?),
        Type::S64 => Val::S64(value.as_i64().context("Expected s64")?),
        Type::Float32 => Val::Float32(value.as_f64().context("Expected f32")? as f32),
        Type::Float64 => Val::Float64(value.as_f64().context("Expected f64")?),
        Type::String => Val::String(value.as_str().context("Expected string")?.to_string()),
        Type::Enum(_) => Val::Enum(value.as_str().context("Expected enum")?.to_string()),
        Type::List(list) => Val::List(
            value
                .as_array()
                .context("Expected list")?
                .iter()
                .map(|v| json_to_val(&list.ty(), v))
                .collect::<Result<_>>()?,
        ),
        Type::Record(record) => Val::Record(
            record
                .fields()
                .map(|field| {
                    Ok((
                        field.name.to_string(),
                        json_to_val(&field.ty, &value[field.name])?,
                    ))
                })
                .collect::<Result<_>>()?,
        ),
        Type::Tuple(tuple) => Val::Tuple(
            tuple
                .types()
                .enumerate()
                .map(|(i, ty)| json_to_val(&ty, &value[i]))
                .collect::<Result<_>>()?,
        ),
        Type::Option(option) => Val::Option(if value.is_null() {
            None
        } else {
            Some(Box::new(json_to_val(&option.ty(), value)?))
        }),
        Type::Variant(variant) => {
            let name = value["case"].as_str().context("Expected variant case")?;
            let case = variant
                .cases()
                .find(|case| case.name == name)
                .context("Unknown variant case")?;
            Val::Variant(
                name.to_string(),
                case.ty
                    .map(|ty| json_to_val(&ty, &value["value"]).map(Box::new))
                    .transpose()?,
            )
        }
        Type::Result(result) => {
            if let Some(value) = value.get("Err") {
                Val::Result(Err(result
                    .err()
                    .map(|ty| json_to_val(&ty, value).map(Box::new))
                    .transpose()?))
            } else {
                let value = value.get("Ok").context("Expected result")?;
                Val::Result(Ok(result
                    .ok()
                    .map(|ty| json_to_val(&ty, value).map(Box::new))
                    .transpose()?))
            }
        }
        _ => bail!("Unsupported host result type {ty:?}"),
    })
}

fn response_json(ty: &Type, response: Result<Value, String>) -> Result<Value> {
    match ty {
        Type::Result(result) => Ok(match response {
            Ok(value) => json!({"Ok": value}),
            Err(error) => {
                json!({"Err": if result.err().is_some() { json!(error) } else { Value::Null }})
            }
        }),
        _ => response.map_err(crate::runtime_error::Error::msg),
    }
}

fn register_future<T>(
    inst: &mut LinkerInstance<'_, ComponentInstanceContext>,
    name: &str,
    method: String,
    ty: Option<Type>,
) -> Result<()>
where
    T: wasmtime::component::Lift
        + wasmtime::component::Lower
        + serde::de::DeserializeOwned
        + Send
        + Sync
        + 'static,
{
    inst.func_new(name, move |mut store, _, params, results| {
        let method = method.clone();
        let ty = ty.clone();
        let args = Value::Array(params.iter().map(val_to_json).collect::<Result<_>>()?);
        let bridge = store.data().bridge.clone();
        let future = FutureReader::new(&mut store, async move {
            let response = bridge.request(method, args).await;
            let value = match ty {
                Some(ty) => response_json(&ty, response)?,
                None => response.map_err(wasmtime::Error::msg)?,
            };
            Ok::<T, wasmtime::Error>(serde_json::from_value(value)?)
        })?;
        results[0] = Val::Future(future.try_into_future_any(&mut store)?);
        Ok(())
    })
}

fn register_import(
    inst: &mut LinkerInstance<'_, ComponentInstanceContext>,
    name: &str,
    method: String,
    types: Vec<Type>,
    is_async: bool,
) -> Result<()> {
    if let [Type::Future(future)] = types.as_slice() {
        let ty = future.ty();
        return match ty.as_ref() {
            None => register_future::<()>(inst, name, method, ty),
            Some(Type::String) => register_future::<String>(inst, name, method, ty),
            Some(Type::S32) => register_future::<i32>(inst, name, method, ty),
            Some(Type::U64) => register_future::<u64>(inst, name, method, ty),
            Some(Type::List(_)) => register_future::<Vec<DeviceInfo>>(inst, name, method, ty),
            Some(Type::Record(record)) => {
                let fields: Vec<_> = record.fields().map(|f| f.name.to_string()).collect();
                if fields.iter().any(|f| f == "clicked-btn-id") {
                    register_future::<DialogResult>(inst, name, method, ty)
                } else if fields.iter().any(|f| f == "width") {
                    register_future::<RenderSize>(inst, name, method, ty)
                } else {
                    register_future::<PickResult>(inst, name, method, ty)
                }
            }
            Some(Type::Result(result)) => match result.ok() {
                None => register_future::<Result<(), ()>>(inst, name, method, ty),
                Some(Type::String) => register_future::<Result<String, ()>>(inst, name, method, ty),
                Some(Type::Record(_)) => {
                    register_future::<Result<SaveSession, ()>>(inst, name, method, ty)
                }
                Some(Type::List(list)) => match list.ty() {
                    Type::U8 => register_future::<Result<Vec<u8>, ()>>(inst, name, method, ty),
                    Type::Record(record) => {
                        if record.fields().any(|f| f.name == "package-name") {
                            register_future::<Result<Vec<AppInfo>, ()>>(inst, name, method, ty)
                        } else {
                            register_future::<Result<Vec<WatchfaceInfo>, ()>>(
                                inst, name, method, ty,
                            )
                        }
                    }
                    _ => bail!("Unsupported legacy result {method}"),
                },
                _ => bail!("Unsupported legacy result {method}"),
            },
            _ => bail!("Unsupported legacy future {method}"),
        };
    }

    if !is_async {
        return inst.func_new_async(name, move |store, _, params, results| {
            let method = method.clone();
            let types = types.clone();
            Box::new(async move {
                if method == "account.get-current" {
                    results[0] = Val::Result(Ok(Some(Box::new(Val::Option(None)))));
                    return Ok(());
                }
                let args = Value::Array(params.iter().map(val_to_json).collect::<Result<_>>()?);
                let response = store.data().bridge.request(method, args).await;
                if let Some(ty) = types.first() {
                    results[0] = json_to_val(ty, &response_json(ty, response)?)?;
                } else {
                    response.map_err(wasmtime::Error::msg)?;
                }
                Ok(())
            })
        });
    }
    inst.func_new_concurrent(name, move |accessor, _, params, results| {
        let method = method.clone();
        let types = types.clone();
        Box::pin(async move {
            if method == "account.get-current" {
                results[0] = Val::Result(Ok(Some(Box::new(Val::Option(None)))));
                return Ok(());
            }
            let args = Value::Array(params.iter().map(val_to_json).collect::<Result<_>>()?);
            let bridge = accessor.with(|mut access| access.get().bridge.clone());
            let response = bridge.request(method, args).await;
            if let Some(ty) = types.first() {
                results[0] = json_to_val(ty, &response_json(ty, response)?)?;
            } else {
                response.map_err(crate::runtime_error::Error::msg)?;
            }
            Ok(())
        })
    })
}

pub fn register_host_interfaces(
    linker: &mut Linker<ComponentInstanceContext>,
    component: &Component,
) -> Result<()> {
    let engine = crate::component::get_engine();
    for (namespace, item) in component.component_type().imports(engine) {
        if !(namespace.starts_with("astrobox:psys-host/")
            || namespace.starts_with("astrobox:psys-host-v4/"))
        {
            continue;
        }
        let interface = namespace
            .split('/')
            .nth(1)
            .unwrap()
            .split('@')
            .next()
            .unwrap();
        if interface == "timer" {
            continue;
        }

        let item = item.ty;
        let ComponentItem::ComponentInstance(imports) = item else {
            continue;
        };
        let mut inst = linker.instance(namespace)?;
        for (name, item) in imports.exports(engine) {
            let item = item.ty;
            if matches!(interface, "ui" | "ui-v3") && name != "get-render-size" {
                continue;
            }
            if let ComponentItem::ComponentFunc(func) = item {
                register_import(
                    &mut inst,
                    name,
                    format!(
                        "{}.{name}",
                        if interface == "ui-v3" {
                            "ui"
                        } else {
                            interface
                        }
                    ),
                    func.results().collect(),
                    func.async_(),
                )?;
            }
        }
    }
    Ok(())
}

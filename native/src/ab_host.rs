use crate::runtime_error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use wasmtime::component::{Linker, LinkerInstance, Resource, ResourceTable, ResourceType};
use wasmtime::StoreContextMut;

use crate::component::ComponentInstanceContext;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ElementNodeDraft {
    pub id: u64,
    #[serde(rename = "type")]
    pub element_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub props: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub styles: HashMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub events: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ElementNodeDraft>,
}

pub struct ElementNodeState {
    pub id: u64,
    pub element_type: ElementType,
    pub content: Option<String>,
    pub props: HashMap<String, String>,
    pub styles: HashMap<String, serde_json::Value>,
    pub events: HashMap<String, String>,
    pub children: Vec<RealElement>,
}

#[derive(Clone)]
pub struct RealElement {
    pub state: Arc<Mutex<ElementNodeState>>,
}

impl RealElement {
    pub fn to_draft(&self) -> ElementNodeDraft {
        let s = self.state.lock().unwrap();
        let children = s.children.iter().map(|c| c.to_draft()).collect();
        ElementNodeDraft {
            id: s.id,
            element_type: s.element_type.as_str().to_string(),
            content: s.content.clone(),
            props: s.props.clone(),
            styles: s.styles.clone(),
            events: s.events.clone(),
            children,
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(enum)]
#[repr(u8)]
pub enum ElementType {
    #[component(name = "BUTTON")]
    Button,
    #[component(name = "INPUT")]
    Input,
    #[component(name = "SELECT")]
    Select,
    #[component(name = "OPTION")]
    Option,
    #[component(name = "IMAGE")]
    Image,
    #[component(name = "VIDEO")]
    Video,
    #[component(name = "AUDIO")]
    Audio,
    #[component(name = "SVG")]
    Svg,
    #[component(name = "DIV")]
    Div,
    #[component(name = "SPAN")]
    Span,
    #[component(name = "P")]
    P,
    #[component(name = "TEXTAREA")]
    Textarea,
    #[component(name = "SWITCH")]
    Switch,
    #[component(name = "SLIDER")]
    Slider,
    #[component(name = "PROGRESS")]
    Progress,
    #[component(name = "GRID")]
    Grid,
    #[component(name = "SCROLL-AREA")]
    ScrollArea,
    #[component(name = "CODE")]
    Code,
    #[component(name = "CARD")]
    Card,
    #[component(name = "TABS-ROOT")]
    TabsRoot,
    #[component(name = "TABS-LIST")]
    TabsList,
    #[component(name = "TABS-TRIGGER")]
    TabsTrigger,
    #[component(name = "TABS-CONTENT")]
    TabsContent,
    #[component(name = "CONTEXT-MENU-ROOT")]
    ContextMenuRoot,
    #[component(name = "CONTEXT-MENU-TRIGGER")]
    ContextMenuTrigger,
    #[component(name = "CONTEXT-MENU-CONTENT")]
    ContextMenuContent,
    #[component(name = "CONTEXT-MENU-ITEM")]
    ContextMenuItem,
    #[component(name = "CONTEXT-MENU-SEPARATOR")]
    ContextMenuSeparator,
    #[component(name = "DIALOG-ROOT")]
    DialogRoot,
    #[component(name = "DIALOG-TRIGGER")]
    DialogTrigger,
    #[component(name = "DIALOG-CONTENT")]
    DialogContent,
    #[component(name = "DIALOG-TITLE")]
    DialogTitle,
    #[component(name = "DIALOG-DESCRIPTION")]
    DialogDescription,
    #[component(name = "DIALOG-CLOSE")]
    DialogClose,
    #[component(name = "DROPDOWN-MENU-ROOT")]
    DropdownMenuRoot,
    #[component(name = "DROPDOWN-MENU-TRIGGER")]
    DropdownMenuTrigger,
    #[component(name = "DROPDOWN-MENU-CONTENT")]
    DropdownMenuContent,
    #[component(name = "DROPDOWN-MENU-ITEM")]
    DropdownMenuItem,
    #[component(name = "DROPDOWN-MENU-SEPARATOR")]
    DropdownMenuSeparator,
    #[component(name = "TOOLTIP")]
    Tooltip,
    #[component(name = "CHECKBOX")]
    Checkbox,
    #[component(name = "SEPARATOR")]
    Separator,
    #[component(name = "BADGE")]
    Badge,
}

impl ElementType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ElementType::Button => "BUTTON",
            ElementType::Input => "INPUT",
            ElementType::Select => "SELECT",
            ElementType::Option => "OPTION",
            ElementType::Image => "IMAGE",
            ElementType::Video => "VIDEO",
            ElementType::Audio => "AUDIO",
            ElementType::Svg => "SVG",
            ElementType::Div => "DIV",
            ElementType::Span => "SPAN",
            ElementType::P => "P",
            ElementType::Textarea => "TEXTAREA",
            ElementType::Switch => "SWITCH",
            ElementType::Slider => "SLIDER",
            ElementType::Progress => "PROGRESS",
            ElementType::Grid => "GRID",
            ElementType::ScrollArea => "SCROLL-AREA",
            ElementType::Code => "CODE",
            ElementType::Card => "CARD",
            ElementType::TabsRoot => "TABS-ROOT",
            ElementType::TabsList => "TABS-LIST",
            ElementType::TabsTrigger => "TABS-TRIGGER",
            ElementType::TabsContent => "TABS-CONTENT",
            ElementType::ContextMenuRoot => "CONTEXT-MENU-ROOT",
            ElementType::ContextMenuTrigger => "CONTEXT-MENU-TRIGGER",
            ElementType::ContextMenuContent => "CONTEXT-MENU-CONTENT",
            ElementType::ContextMenuItem => "CONTEXT-MENU-ITEM",
            ElementType::ContextMenuSeparator => "CONTEXT-MENU-SEPARATOR",
            ElementType::DialogRoot => "DIALOG-ROOT",
            ElementType::DialogTrigger => "DIALOG-TRIGGER",
            ElementType::DialogContent => "DIALOG-CONTENT",
            ElementType::DialogTitle => "DIALOG-TITLE",
            ElementType::DialogDescription => "DIALOG-DESCRIPTION",
            ElementType::DialogClose => "DIALOG-CLOSE",
            ElementType::DropdownMenuRoot => "DROPDOWN-MENU-ROOT",
            ElementType::DropdownMenuTrigger => "DROPDOWN-MENU-TRIGGER",
            ElementType::DropdownMenuContent => "DROPDOWN-MENU-CONTENT",
            ElementType::DropdownMenuItem => "DROPDOWN-MENU-ITEM",
            ElementType::DropdownMenuSeparator => "DROPDOWN-MENU-SEPARATOR",
            ElementType::Tooltip => "TOOLTIP",
            ElementType::Checkbox => "CHECKBOX",
            ElementType::Separator => "SEPARATOR",
            ElementType::Badge => "BADGE",
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(enum)]
#[repr(u8)]
pub enum ElementTypeV2 {
    #[component(name = "BUTTON")]
    Button,
    #[component(name = "INPUT")]
    Input,
    #[component(name = "SELECT")]
    Select,
    #[component(name = "OPTION")]
    Option,
    #[component(name = "IMAGE")]
    Image,
    #[component(name = "VIDEO")]
    Video,
    #[component(name = "AUDIO")]
    Audio,
    #[component(name = "SVG")]
    Svg,
    #[component(name = "DIV")]
    Div,
    #[component(name = "SPAN")]
    Span,
    #[component(name = "P")]
    P,
}

impl ElementTypeV2 {
    pub fn to_v4(&self) -> ElementType {
        match self {
            ElementTypeV2::Button => ElementType::Button,
            ElementTypeV2::Input => ElementType::Input,
            ElementTypeV2::Select => ElementType::Select,
            ElementTypeV2::Option => ElementType::Option,
            ElementTypeV2::Image => ElementType::Image,
            ElementTypeV2::Video => ElementType::Video,
            ElementTypeV2::Audio => ElementType::Audio,
            ElementTypeV2::Svg => ElementType::Svg,
            ElementTypeV2::Div => ElementType::Div,
            ElementTypeV2::Span => ElementType::Span,
            ElementTypeV2::P => ElementType::P,
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(enum)]
#[repr(u8)]
pub enum UiEvent {
    #[component(name = "CLICK")]
    Click,
    #[component(name = "HOVER")]
    Hover,
    #[component(name = "CHANGE")]
    Change,
    #[component(name = "INPUT")]
    Input,
    #[component(name = "FOCUS")]
    Focus,
    #[component(name = "BLUR")]
    Blur,
    #[component(name = "MOUSE-ENTER")]
    MouseEnter,
    #[component(name = "MOUSE-LEAVE")]
    MouseLeave,
    #[component(name = "POINTER-DOWN")]
    PointerDown,
    #[component(name = "POINTER-UP")]
    PointerUp,
    #[component(name = "POINTER-MOVE")]
    PointerMove,
    #[component(name = "KEY-DOWN")]
    KeyDown,
    #[component(name = "KEY-UP")]
    KeyUp,
    #[component(name = "LONG-PRESS")]
    LongPress,
}

impl UiEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            UiEvent::Click => "CLICK",
            UiEvent::Hover => "HOVER",
            UiEvent::Change => "CHANGE",
            UiEvent::Input => "INPUT",
            UiEvent::Focus => "FOCUS",
            UiEvent::Blur => "BLUR",
            UiEvent::MouseEnter => "MOUSE-ENTER",
            UiEvent::MouseLeave => "MOUSE-LEAVE",
            UiEvent::PointerDown => "POINTER-DOWN",
            UiEvent::PointerUp => "POINTER-UP",
            UiEvent::PointerMove => "POINTER-MOVE",
            UiEvent::KeyDown => "KEY-DOWN",
            UiEvent::KeyUp => "KEY-UP",
            UiEvent::LongPress => "LONG-PRESS",
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(enum)]
#[repr(u8)]
pub enum UiEventV2 {
    #[component(name = "CLICK")]
    Click,
    #[component(name = "HOVER")]
    Hover,
    #[component(name = "CHANGE")]
    Change,
    #[component(name = "INPUT")]
    Input,
    #[component(name = "FOCUS")]
    Focus,
    #[component(name = "BLUR")]
    Blur,
    #[component(name = "MOUSE-ENTER")]
    MouseEnter,
    #[component(name = "MOUSE-LEAVE")]
    MouseLeave,
    #[component(name = "POINTER-DOWN")]
    PointerDown,
    #[component(name = "POINTER-UP")]
    PointerUp,
    #[component(name = "POINTER-MOVE")]
    PointerMove,
}

impl UiEventV2 {
    pub fn as_str(&self) -> &'static str {
        match self {
            UiEventV2::Click => "CLICK",
            UiEventV2::Hover => "HOVER",
            UiEventV2::Change => "CHANGE",
            UiEventV2::Input => "INPUT",
            UiEventV2::Focus => "FOCUS",
            UiEventV2::Blur => "BLUR",
            UiEventV2::MouseEnter => "MOUSE-ENTER",
            UiEventV2::MouseLeave => "MOUSE-LEAVE",
            UiEventV2::PointerDown => "POINTER-DOWN",
            UiEventV2::PointerUp => "POINTER-UP",
            UiEventV2::PointerMove => "POINTER-MOVE",
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(enum)]
#[repr(u8)]
pub enum FlexDirection {
    #[component(name = "ROW")]
    Row,
    #[component(name = "COLUMN")]
    Column,
    #[component(name = "ROW-REVERSE")]
    RowReverse,
    #[component(name = "COLUMN-REVERSE")]
    ColumnReverse,
}

impl FlexDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlexDirection::Row => "ROW",
            FlexDirection::Column => "COLUMN",
            FlexDirection::RowReverse => "ROW-REVERSE",
            FlexDirection::ColumnReverse => "COLUMN-REVERSE",
        }
    }
}

#[derive(
    Clone,
    Debug,
    serde::Deserialize,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
#[serde(rename_all = "kebab-case")]
pub struct DialogResult {
    #[component(name = "clicked-btn-id")]
    pub clicked_btn_id: String,
    #[component(name = "input-result")]
    pub input_result: String,
}

#[derive(
    Copy,
    Clone,
    Debug,
    serde::Deserialize,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
#[serde(rename_all = "kebab-case")]
pub struct RenderSize {
    pub width: u32,
    pub height: u32,
}

#[derive(
    Clone,
    Debug,
    serde::Deserialize,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
#[serde(rename_all = "kebab-case")]
pub struct PickResult {
    pub name: String,
    pub data: Vec<u8>,
}

#[derive(
    Clone,
    Debug,
    serde::Deserialize,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
#[serde(rename_all = "kebab-case")]
pub struct SaveSession {
    #[component(name = "session-id")]
    pub session_id: u64,
    pub name: String,
}

#[derive(
    Clone,
    Debug,
    serde::Deserialize,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
#[serde(rename_all = "kebab-case")]
pub struct DeviceInfo {
    pub name: String,
    pub addr: String,
}

#[derive(
    Clone,
    Debug,
    serde::Deserialize,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
#[serde(rename_all = "kebab-case")]
pub struct AppInfo {
    #[component(name = "package-name")]
    pub package_name: String,
    pub fingerprint: Vec<u32>,
    #[component(name = "version-code")]
    pub version_code: u32,
    #[component(name = "can-remove")]
    pub can_remove: bool,
    #[component(name = "app-name")]
    pub app_name: String,
}

#[derive(
    Clone,
    Debug,
    serde::Deserialize,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
#[serde(rename_all = "kebab-case")]
pub struct WatchfaceInfo {
    pub id: String,
    pub name: String,
    #[component(name = "is-current")]
    pub is_current: bool,
}

pub fn register_ab_host_apis(
    linker: &mut Linker<ComponentInstanceContext>,
    component: &wasmtime::component::Component,
) -> Result<()> {
    register_ui_v4(linker)?;
    register_ui_v3(linker)?;
    register_ui_v2(linker)?;
    register_timer_v4(linker)?;
    register_timer_v2(linker)?;
    crate::component_host::register_host_interfaces(linker, component)
}

fn register_timer_v4(linker: &mut Linker<ComponentInstanceContext>) -> Result<()> {
    let mut inst = linker.instance("astrobox:psys-host-v4/timer")?;
    for (name, repeat) in [("set-timeout", false), ("set-interval", true)] {
        inst.func_wrap(
            name,
            move |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
                  (delay, payload): (u64, String)|
                  -> Result<(u64,)> {
                Ok((crate::component::start_component_timer(
                    ctx.data_mut(),
                    delay,
                    payload,
                    repeat,
                ),))
            },
        )?;
    }
    inst.func_wrap(
        "clear-timer",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>, (id,): (u64,)| -> Result<()> {
            if let Some(timer) = ctx.data_mut().timers.remove(&id) {
                timer.abort();
            }
            Ok(())
        },
    )?;
    Ok(())
}

fn register_timer_v2(linker: &mut Linker<ComponentInstanceContext>) -> Result<()> {
    let mut inst = linker.instance("astrobox:psys-host/timer")?;
    for (name, repeat) in [("set-timeout", false), ("set-interval", true)] {
        inst.func_new(name, move |mut store, _, params, results| {
            let [wasmtime::component::Val::U64(delay), wasmtime::component::Val::String(payload)] =
                params
            else {
                wasmtime::bail!("invalid timer arguments");
            };
            let id = crate::component::start_component_timer(
                store.data_mut(),
                *delay,
                payload.clone(),
                repeat,
            );
            let future = wasmtime::component::FutureReader::new(&mut store, async move {
                Ok::<u64, wasmtime::Error>(id)
            })?;
            results[0] = wasmtime::component::Val::Future(future.try_into_future_any(&mut store)?);
            Ok(())
        })?;
    }
    inst.func_new("clear-timer", |mut store, _, params, results| {
        let [wasmtime::component::Val::U64(id)] = params else {
            wasmtime::bail!("invalid timer id");
        };
        if let Some(timer) = store.data_mut().timers.remove(id) {
            timer.abort();
        }
        let future = wasmtime::component::FutureReader::new(&mut store, async {
            Ok::<(), wasmtime::Error>(())
        })?;
        results[0] = wasmtime::component::Val::Future(future.try_into_future_any(&mut store)?);
        Ok(())
    })?;
    Ok(())
}

// Owned arguments leave the guest resource table and must leave the Host table too.
fn take_or_clone_element(
    table: &mut ResourceTable,
    element: Resource<RealElement>,
) -> Result<RealElement> {
    if element.owned() {
        Ok(table.delete(element)?)
    } else {
        Ok(table.get(&element)?.clone())
    }
}

fn return_owned_element(
    table: &mut ResourceTable,
    element: Resource<RealElement>,
) -> Result<Resource<RealElement>> {
    if element.owned() {
        Ok(element)
    } else {
        let value = table.get(&element)?.clone();
        Ok(table.push(value)?)
    }
}

fn register_element_common_methods(
    inst: &mut LinkerInstance<ComponentInstanceContext>,
) -> Result<()> {
    inst.func_wrap(
        "[method]element.content",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, content): (Resource<RealElement>, Option<String>)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state.lock().unwrap().content = content;
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.prop",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, name, val): (Resource<RealElement>, String, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state.lock().unwrap().props.insert(name, val);
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.flex",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("display".to_string(), serde_json::json!("flex"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.flex-direction",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, dir): (Resource<RealElement>, FlexDirection)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state.lock().unwrap().styles.insert(
                "flex_direction".to_string(),
                serde_json::json!(dir.as_str()),
            );
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.margin",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("margin".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.margin-top",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("margin_top".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.margin-bottom",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("margin_bottom".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.margin-left",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("margin_left".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.margin-right",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("margin_right".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.padding",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("padding".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.padding-top",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("padding_top".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.padding-bottom",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("padding_bottom".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.padding-left",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("padding_left".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.padding-right",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("padding_right".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.align-center",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("align_items".to_string(), serde_json::json!("center"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.align-start",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("align_items".to_string(), serde_json::json!("flex-start"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.align-end",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("align_items".to_string(), serde_json::json!("flex-end"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.justify-center",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("justify_content".to_string(), serde_json::json!("center"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.justify-start",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state.lock().unwrap().styles.insert(
                "justify_content".to_string(),
                serde_json::json!("flex-start"),
            );
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.justify-end",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("justify_content".to_string(), serde_json::json!("flex-end"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.bg",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, c): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("background_color".to_string(), serde_json::json!(c));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.text-color",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, c): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("color".to_string(), serde_json::json!(c));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.size",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, s): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            let mut st = elem.state.lock().unwrap();
            st.styles
                .insert("font_size".to_string(), serde_json::json!(s));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.width",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, w): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("width".to_string(), serde_json::json!(w));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.width-full",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("width".to_string(), serde_json::json!("100%"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.width-half",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("width".to_string(), serde_json::json!("50%"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.height",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, h): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("height".to_string(), serde_json::json!(h));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.height-full",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("height".to_string(), serde_json::json!("100%"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.height-half",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("height".to_string(), serde_json::json!("50%"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.radius",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, r): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("border_radius".to_string(), serde_json::json!(r));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.border",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, w, c): (Resource<RealElement>, u32, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            let mut st = elem.state.lock().unwrap();
            st.styles
                .insert("border_width".to_string(), serde_json::json!(w));
            st.styles
                .insert("border_color".to_string(), serde_json::json!(c));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.relative",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("position".to_string(), serde_json::json!("relative"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.absolute",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("position".to_string(), serde_json::json!("absolute"));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.top",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("top".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.bottom",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("bottom".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.left",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("left".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.right",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("right".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.opacity",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, o): (Resource<RealElement>, f32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("opacity".to_string(), serde_json::json!(o));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.font-size",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, s): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("font_size".to_string(), serde_json::json!(s));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.font-weight",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, w): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("font_weight".to_string(), serde_json::json!(w));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.font-family",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, f): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("font_family".to_string(), serde_json::json!(f));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.text-align",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, a): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("text_align".to_string(), serde_json::json!(a));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.line-height",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, h): (Resource<RealElement>, f32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("line_height".to_string(), serde_json::json!(h));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.box-shadow",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, s): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("box_shadow".to_string(), serde_json::json!(s));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.cursor",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, c): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("cursor".to_string(), serde_json::json!(c));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.display",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, d): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("display".to_string(), serde_json::json!(d));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.justify-content",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, j): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("justify_content".to_string(), serde_json::json!(j));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.align-items",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, a): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("align_items".to_string(), serde_json::json!(a));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.align-self",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, a): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("align_self".to_string(), serde_json::json!(a));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.overflow",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, o): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("overflow".to_string(), serde_json::json!(o));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.overflow-x",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, o): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("overflow_x".to_string(), serde_json::json!(o));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.overflow-y",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, o): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("overflow_y".to_string(), serde_json::json!(o));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.position",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, p): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("position".to_string(), serde_json::json!(p));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.without-default-styles",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state.lock().unwrap().styles.insert(
                "without_default_styles".to_string(),
                serde_json::json!(true),
            );
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.autofocus",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .props
                .insert("autofocus".to_string(), "true".to_string());
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.tab-index",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, i): (Resource<RealElement>, i32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .props
                .insert("tab_index".to_string(), i.to_string());
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.z-index",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, z): (Resource<RealElement>, i32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("z_index".to_string(), serde_json::json!(z));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.disabled",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res,): (Resource<RealElement>,)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .props
                .insert("disabled".to_string(), "true".to_string());
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.child",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, child_res): (Resource<RealElement>, Resource<RealElement>)|
         -> Result<(Resource<RealElement>,)> {
            let parent = ctx.data_mut().table.get(&self_res)?.clone();
            let child = take_or_clone_element(&mut ctx.data_mut().table, child_res)?;
            parent.state.lock().unwrap().children.push(child);
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.grid-template-columns",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, c): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("grid_template_columns".to_string(), serde_json::json!(c));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.gap",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, g): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("gap".to_string(), serde_json::json!(g));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.max-width",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, w): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("max_width".to_string(), serde_json::json!(w));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.max-height",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, h): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("max_height".to_string(), serde_json::json!(h));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.min-width",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, w): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("min_width".to_string(), serde_json::json!(w));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.min-height",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, h): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("min_height".to_string(), serde_json::json!(h));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.scroll-top",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, p): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("scroll_top".to_string(), serde_json::json!(p));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.scroll-left",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, p): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("scroll_left".to_string(), serde_json::json!(p));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.scroll-to",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, t, l): (Resource<RealElement>, u32, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            let mut st = elem.state.lock().unwrap();
            st.styles
                .insert("scroll_top".to_string(), serde_json::json!(t));
            st.styles
                .insert("scroll_left".to_string(), serde_json::json!(l));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.scroll-behavior",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, b): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("scroll_behavior".to_string(), serde_json::json!(b));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.flex-grow",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, f32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("flex_grow".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.flex-shrink",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, f32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("flex_shrink".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.transition",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("transition".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.transform",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("transform".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.transform-origin",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("transform_origin".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("animation".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation-name",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("animation_name".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation-duration-ms",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("animation_duration_ms".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation-delay-ms",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, u32)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("animation_delay_ms".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation-easing",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("animation_easing".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation-iteration-count",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state.lock().unwrap().styles.insert(
                "animation_iteration_count".to_string(),
                serde_json::json!(v),
            );
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation-direction",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("animation_direction".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation-fill-mode",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("animation_fill_mode".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation-play-state",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("animation_play_state".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.animation-preset",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("animation_preset".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.will-change",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("will_change".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.filter",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("filter".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.backdrop-filter",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("backdrop_filter".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.perspective",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("perspective".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "[method]element.backface-visibility",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, v): (Resource<RealElement>, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .styles
                .insert("backface_visibility".to_string(), serde_json::json!(v));
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    inst.func_wrap(
        "render",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (id, el): (String, Resource<RealElement>)|
         -> Result<()> {
            let elem = take_or_clone_element(&mut ctx.data_mut().table, el)?;
            let draft = elem.to_draft();
            let payload = serde_json::json!({ "target_id": id, "root": draft });
            let json_str = serde_json::to_string(&payload)?;
            ctx.data().publish_ui(json_str);
            Ok(())
        },
    )?;
    inst.func_wrap("render-to-text-card", |ctx: StoreContextMut<'_, ComponentInstanceContext>, (id, text): (String, String)| -> Result<()> {
        let payload = serde_json::json!({
            "target_id": id,
            "root": {
                "id": 0, "type": "CARD", "content": null, "props": {}, "styles": { "padding": 16 }, "events": {},
                "children": [{ "id": 1, "type": "P", "content": text, "props": {}, "styles": {}, "events": {}, "children": [] }]
            }
        });
        let json_str = serde_json::to_string(&payload)?;
        ctx.data().publish_ui(json_str);
        Ok(())
    })?;

    Ok(())
}

fn register_ui_v4(linker: &mut Linker<ComponentInstanceContext>) -> Result<()> {
    let mut inst = linker.instance("astrobox:psys-host-v4/ui")?;
    inst.resource(
        "element",
        ResourceType::host::<RealElement>(),
        |mut ctx, rep| {
            ctx.data_mut()
                .table
                .delete(Resource::<RealElement>::new_own(rep))?;
            Ok(())
        },
    )?;
    inst.func_wrap(
        "[constructor]element",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (elem_type, content): (ElementType, Option<String>)|
         -> Result<(Resource<RealElement>,)> {
            let id = ctx.data().next_node_id.fetch_add(1, Ordering::SeqCst);
            let elem = RealElement {
                state: Arc::new(Mutex::new(ElementNodeState {
                    id,
                    element_type: elem_type,
                    content,
                    props: HashMap::new(),
                    styles: HashMap::new(),
                    events: HashMap::new(),
                    children: Vec::new(),
                })),
            };
            let res = ctx.data_mut().table.push(elem)?;
            Ok((res,))
        },
    )?;
    inst.func_wrap(
        "[method]element.on",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, event, id): (Resource<RealElement>, UiEvent, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .events
                .insert(event.as_str().to_string(), id);
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    register_element_common_methods(&mut inst)?;

    Ok(())
}

fn register_ui_v3(linker: &mut Linker<ComponentInstanceContext>) -> Result<()> {
    let mut inst = linker.instance("astrobox:psys-host/ui-v3")?;
    inst.resource(
        "element",
        ResourceType::host::<RealElement>(),
        |mut ctx, rep| {
            ctx.data_mut()
                .table
                .delete(Resource::<RealElement>::new_own(rep))?;
            Ok(())
        },
    )?;
    inst.func_wrap(
        "[constructor]element",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (elem_type, content): (ElementType, Option<String>)|
         -> Result<(Resource<RealElement>,)> {
            let id = ctx.data().next_node_id.fetch_add(1, Ordering::SeqCst);
            let elem = RealElement {
                state: Arc::new(Mutex::new(ElementNodeState {
                    id,
                    element_type: elem_type,
                    content,
                    props: HashMap::new(),
                    styles: HashMap::new(),
                    events: HashMap::new(),
                    children: Vec::new(),
                })),
            };
            let res = ctx.data_mut().table.push(elem)?;
            Ok((res,))
        },
    )?;
    inst.func_wrap(
        "[method]element.on",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, event, id): (Resource<RealElement>, UiEvent, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .events
                .insert(event.as_str().to_string(), id);
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    register_element_common_methods(&mut inst)?;

    Ok(())
}

fn register_ui_v2(linker: &mut Linker<ComponentInstanceContext>) -> Result<()> {
    let mut inst = linker.instance("astrobox:psys-host/ui")?;
    inst.resource(
        "element",
        ResourceType::host::<RealElement>(),
        |mut ctx, rep| {
            ctx.data_mut()
                .table
                .delete(Resource::<RealElement>::new_own(rep))?;
            Ok(())
        },
    )?;
    inst.func_wrap(
        "[constructor]element",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (elem_type, content): (ElementTypeV2, Option<String>)|
         -> Result<(Resource<RealElement>,)> {
            let id = ctx.data().next_node_id.fetch_add(1, Ordering::SeqCst);
            let elem = RealElement {
                state: Arc::new(Mutex::new(ElementNodeState {
                    id,
                    element_type: elem_type.to_v4(),
                    content,
                    props: HashMap::new(),
                    styles: HashMap::new(),
                    events: HashMap::new(),
                    children: Vec::new(),
                })),
            };
            let res = ctx.data_mut().table.push(elem)?;
            Ok((res,))
        },
    )?;
    inst.func_wrap(
        "[method]element.on",
        |mut ctx: StoreContextMut<'_, ComponentInstanceContext>,
         (self_res, event, id): (Resource<RealElement>, UiEventV2, String)|
         -> Result<(Resource<RealElement>,)> {
            let elem = ctx.data_mut().table.get(&self_res)?.clone();
            elem.state
                .lock()
                .unwrap()
                .events
                .insert(event.as_str().to_string(), id);
            Ok((return_owned_element(&mut ctx.data_mut().table, self_res)?,))
        },
    )?;
    register_element_common_methods(&mut inst)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityParamType {
    String,
    Path,
    Bool,
    /// JSON-safe value: any value that can round-trip through JSON
    /// (objects, arrays, strings, numbers, booleans, null).
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityParam {
    pub name: &'static str,
    pub ty: CapabilityParamType,
    /// Whether this parameter is required for deterministic exec stages.
    /// Optional params can be omitted; the tool handler provides defaults.
    /// Default: true (required).
    pub required: bool,
}

impl CapabilityParam {
    pub const fn required(name: &'static str, ty: CapabilityParamType) -> Self {
        Self {
            name,
            ty,
            required: true,
        }
    }
    pub const fn optional(name: &'static str, ty: CapabilityParamType) -> Self {
        Self {
            name,
            ty,
            required: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySpec {
    pub name: &'static str,
    pub required_params: &'static [CapabilityParam],
}

impl CapabilitySpec {
    pub fn has_required_param(&self, name: &str) -> bool {
        self.required_params
            .iter()
            .any(|param| param.name == name && param.required)
    }

    /// Check whether a parameter name is valid for this capability
    /// (required or optional).
    pub fn has_param(&self, name: &str) -> bool {
        self.required_params.iter().any(|param| param.name == name)
    }
}

const FS_READ_PARAMS: &[CapabilityParam] =
    &[CapabilityParam::required("path", CapabilityParamType::Path)];

const FS_WRITE_PARAMS: &[CapabilityParam] = &[
    CapabilityParam::required("path", CapabilityParamType::Path),
    CapabilityParam::required("content", CapabilityParamType::String),
];

const OS_SHELL_PARAMS: &[CapabilityParam] = &[CapabilityParam::required(
    "command",
    CapabilityParamType::String,
)];

const USER_ELICIT_PARAMS: &[CapabilityParam] = &[CapabilityParam::required(
    "question",
    CapabilityParamType::String,
)];

const USER_CONFIRM_PARAMS: &[CapabilityParam] = &[CapabilityParam::required(
    "message",
    CapabilityParamType::String,
)];

pub const FS_READ: CapabilitySpec = CapabilitySpec {
    name: "fs.read",
    required_params: FS_READ_PARAMS,
};

pub const FS_WRITE: CapabilitySpec = CapabilitySpec {
    name: "fs.write",
    required_params: FS_WRITE_PARAMS,
};

pub const OS_SHELL: CapabilitySpec = CapabilitySpec {
    name: "os.shell",
    required_params: OS_SHELL_PARAMS,
};

pub const USER_ELICIT: CapabilitySpec = CapabilitySpec {
    name: "user.elicit",
    required_params: USER_ELICIT_PARAMS,
};

pub const USER_CONFIRM: CapabilitySpec = CapabilitySpec {
    name: "user.confirm",
    required_params: USER_CONFIRM_PARAMS,
};

// --- Browser-native capabilities (web target only) ---

// Only `url` and `method` are catalog-required (policy-bindable).
// `headers` and `body` are optional — tool provides defaults.
const HTTP_FETCH_PARAMS: &[CapabilityParam] = &[
    CapabilityParam::required("url", CapabilityParamType::String),
    CapabilityParam::required("method", CapabilityParamType::String),
    CapabilityParam::optional("headers", CapabilityParamType::Json),
    CapabilityParam::optional("body", CapabilityParamType::Json),
];

const BROWSER_STORAGE_READ_PARAMS: &[CapabilityParam] = &[CapabilityParam::required(
    "key",
    CapabilityParamType::String,
)];

const BROWSER_STORAGE_WRITE_PARAMS: &[CapabilityParam] = &[
    CapabilityParam::required("key", CapabilityParamType::String),
    CapabilityParam::required("value", CapabilityParamType::Json),
];

const BROWSER_JS_RUN_PARAMS: &[CapabilityParam] = &[
    CapabilityParam::required("code", CapabilityParamType::String),
    CapabilityParam::required("input", CapabilityParamType::Json),
];

// Dynamic source for this capability is intentionally handled only by the
// web target's opaque-origin sandbox. Unlike `browser.js.run`, its `code`
// argument may be a workflow input or a prior stage output.
const BROWSER_JS_SANDBOX_PARAMS: &[CapabilityParam] = &[
    CapabilityParam::required("code", CapabilityParamType::String),
    CapabilityParam::required("input", CapabilityParamType::Json),
];

pub const HTTP_FETCH: CapabilitySpec = CapabilitySpec {
    name: "http.fetch",
    required_params: HTTP_FETCH_PARAMS,
};

pub const BROWSER_STORAGE_READ: CapabilitySpec = CapabilitySpec {
    name: "browser.storage.read",
    required_params: BROWSER_STORAGE_READ_PARAMS,
};

pub const BROWSER_STORAGE_WRITE: CapabilitySpec = CapabilitySpec {
    name: "browser.storage.write",
    required_params: BROWSER_STORAGE_WRITE_PARAMS,
};

pub const BROWSER_JS_RUN: CapabilitySpec = CapabilitySpec {
    name: "browser.js.run",
    required_params: BROWSER_JS_RUN_PARAMS,
};

pub const BROWSER_JS_SANDBOX: CapabilitySpec = CapabilitySpec {
    name: "browser.js.sandbox",
    required_params: BROWSER_JS_SANDBOX_PARAMS,
};

pub fn get_capability(name: &str) -> Option<&'static CapabilitySpec> {
    match name {
        "fs.read" => Some(&FS_READ),
        "fs.write" => Some(&FS_WRITE),
        "os.shell" => Some(&OS_SHELL),
        "user.elicit" => Some(&USER_ELICIT),
        "user.confirm" => Some(&USER_CONFIRM),
        "http.fetch" => Some(&HTTP_FETCH),
        "browser.storage.read" => Some(&BROWSER_STORAGE_READ),
        "browser.storage.write" => Some(&BROWSER_STORAGE_WRITE),
        "browser.js.run" => Some(&BROWSER_JS_RUN),
        "browser.js.sandbox" => Some(&BROWSER_JS_SANDBOX),
        _ => None,
    }
}

pub fn is_known_capability(name: &str) -> bool {
    get_capability(name).is_some()
}

/// Return the type of a trigger-bound variable for a given capability.
///
/// Returns `None` when the capability or parameter is unknown.
/// Used by IR validation to type-check policy expression method calls.
pub fn bound_var_type(capability: &str, param_name: &str) -> Option<CapabilityParamType> {
    let spec = get_capability(capability)?;
    spec.required_params
        .iter()
        .find(|p| p.name == param_name)
        .map(|p| p.ty)
}

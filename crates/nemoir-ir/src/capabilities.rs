#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityParamType {
    String,
    Path,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityParam {
    pub name: &'static str,
    pub ty: CapabilityParamType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySpec {
    pub name: &'static str,
    pub required_params: &'static [CapabilityParam],
}

impl CapabilitySpec {
    pub fn has_required_param(&self, name: &str) -> bool {
        self.required_params.iter().any(|param| param.name == name)
    }
}

const FS_READ_PARAMS: &[CapabilityParam] = &[CapabilityParam {
    name: "path",
    ty: CapabilityParamType::Path,
}];

const FS_WRITE_PARAMS: &[CapabilityParam] = &[
    CapabilityParam {
        name: "path",
        ty: CapabilityParamType::Path,
    },
    CapabilityParam {
        name: "content",
        ty: CapabilityParamType::String,
    },
];

const OS_SHELL_PARAMS: &[CapabilityParam] = &[CapabilityParam {
    name: "command",
    ty: CapabilityParamType::String,
}];

const USER_ELICIT_PARAMS: &[CapabilityParam] = &[CapabilityParam {
    name: "question",
    ty: CapabilityParamType::String,
}];

const USER_CONFIRM_PARAMS: &[CapabilityParam] = &[CapabilityParam {
    name: "message",
    ty: CapabilityParamType::String,
}];

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

pub fn get_capability(name: &str) -> Option<&'static CapabilitySpec> {
    match name {
        "fs.read" => Some(&FS_READ),
        "fs.write" => Some(&FS_WRITE),
        "os.shell" => Some(&OS_SHELL),
        "user.elicit" => Some(&USER_ELICIT),
        "user.confirm" => Some(&USER_CONFIRM),
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

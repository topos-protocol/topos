use std::{collections::HashMap, str::FromStr};

use opentelemetry::{
    global,
    propagation::{Extractor, Injector},
    Context,
};
use serde::{Deserialize, Serialize};
use tonic::metadata::MetadataKey;

pub struct TonicMetaInjector<'a>(pub &'a mut tonic::metadata::MetadataMap);
pub struct TonicMetaExtractor<'a>(pub &'a tonic::metadata::MetadataMap);

impl<'a> TonicMetaExtractor<'a> {
    pub fn extract(&self) -> opentelemetry::Context {
        global::get_text_map_propagator(|propagator| propagator.extract(self))
    }
}

impl<'a> TonicMetaInjector<'a> {
    pub fn inject(&mut self, context: &Context) {
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(context, self);
        })
    }
}

impl<'a> Injector for TonicMetaInjector<'a> {
    /// Set a key and value in the MetadataMap.  Does nothing if the key or value are not valid inputs
    fn set(&mut self, key: &str, value: String) {
        if let Ok(key) = MetadataKey::from_str(key) {
            if let Ok(val) = value.parse() {
                self.0.insert(key, val);
            } else {
                tracing::warn!("Invalid value: {}", value);
            }
        } else {
            tracing::warn!("Invalid key: {}", key);
        }
    }
}

impl<'a> Extractor for TonicMetaExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0
            .keys()
            .map(|k| match k {
                tonic::metadata::KeyRef::Ascii(k) => k.as_str(),
                tonic::metadata::KeyRef::Binary(k) => k.as_str(),
            })
            .collect()
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PropagationContext {
    context: HashMap<String, String>,
}

impl PropagationContext {
    pub fn inject(context: &Context) -> Self {
        global::get_text_map_propagator(|propagator| {
            let mut propagation_context = PropagationContext::default();
            propagator.inject_context(context, &mut propagation_context);
            propagation_context
        })
    }

    pub fn extract(&self) -> opentelemetry::Context {
        global::get_text_map_propagator(|propagator| propagator.extract(self))
    }
}

impl Injector for PropagationContext {
    fn set(&mut self, key: &str, value: String) {
        self.context.insert(key.to_string(), value);
    }
}

impl Extractor for PropagationContext {
    fn get(&self, key: &str) -> Option<&str> {
        self.context.get(key).map(|s| s.as_ref())
    }

    fn keys(&self) -> Vec<&str> {
        self.context.keys().map(|k| k.as_ref()).collect()
    }
}

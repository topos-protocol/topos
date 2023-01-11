use std::collections::HashMap;

use opentelemetry::{
    global,
    propagation::{Extractor, Injector},
    Context,
};
use serde::{Deserialize, Serialize};

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

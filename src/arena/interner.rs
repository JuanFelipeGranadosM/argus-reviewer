use std::collections::HashMap;

use super::ArenaError;

/// Identificador de un nombre deduplicado en [`StringInterner`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct NameId(pub u32);

/// Deduplica identificadores: cada texto distinto se guarda una vez y se referencia por `NameId`.
#[derive(Debug, Default)]
pub struct StringInterner {
    ids: HashMap<Box<str>, NameId>,
    names: Vec<Box<str>>,
}

impl StringInterner {
    pub fn intern(&mut self, name: &str) -> Result<NameId, ArenaError> {
        if let Some(&id) = self.ids.get(name) {
            return Ok(id);
        }
        let id = NameId(u32::try_from(self.names.len()).map_err(|_| ArenaError::Full("names"))?);
        self.names.push(name.into());
        self.ids.insert(name.into(), id);
        Ok(id)
    }

    pub fn resolve(&self, id: NameId) -> Option<&str> {
        self.names.get(id.0 as usize).map(|name| &**name)
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_text_returns_same_id() {
        let mut interner = StringInterner::default();
        let a = interner.intern("payload").unwrap();
        let b = interner.intern("total").unwrap();
        assert_eq!(interner.intern("payload").unwrap(), a);
        assert_ne!(a, b);
        assert_eq!(interner.resolve(b), Some("total"));
        assert_eq!(interner.len(), 2);
    }
}

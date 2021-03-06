///types and functions used by the compiler
use crate::builtin::*;

use crate::context::*;
use crate::globals::Globals;
use crate::value::*;

use std::collections::HashMap;

use crate::compiler::{BUILTIN_STORAGE, NULL_STORAGE};

pub type StoredValue = usize; //index to stored value in globals.stored_values

pub struct ValStorage {
    pub map: HashMap<usize, StoredValData>, //val, fn context, mutable, lifetime
}

#[derive(Debug, Clone)]
pub struct StoredValData {
    pub val: Value,
    pub fn_context: Group,
    pub mutable: bool,
    pub lifetime: u16,
}
/*
LIFETIME:

value gets deleted when lifetime reaches 0
deeper scope => lifetime++
shallower scopr => lifetime--
*/

impl std::ops::Index<usize> for ValStorage {
    type Output = Value;

    fn index(&self, i: usize) -> &Self::Output {
        &self
            .map
            .get(&i)
            .unwrap_or_else(|| panic!("index {} not found", i))
            .val
    }
}

impl std::ops::IndexMut<usize> for ValStorage {
    fn index_mut(&mut self, i: usize) -> &mut Self::Output {
        &mut self.map.get_mut(&i).unwrap().val
    }
}

use std::collections::HashSet;
impl ValStorage {
    pub fn new() -> Self {
        ValStorage {
            map: vec![
                (
                    BUILTIN_STORAGE,
                    StoredValData {
                        val: Value::Builtins,
                        fn_context: Group::new(0),
                        mutable: false,
                        lifetime: 1,
                    },
                ),
                (
                    NULL_STORAGE,
                    StoredValData {
                        val: Value::Null,
                        fn_context: Group::new(0),
                        mutable: false,
                        lifetime: 1,
                    },
                ),
            ]
            .iter()
            .cloned()
            .collect(),
        }
    }

    pub fn set_mutability(&mut self, index: usize, mutable: bool) {
        if !mutable || !matches!(self[index], Value::Macro(_)) {
            (*self.map.get_mut(&index).unwrap()).mutable = mutable;
        }

        match self[index].clone() {
            Value::Array(a) => {
                for e in a {
                    self.set_mutability(e, mutable);
                }
            }
            Value::Dict(a) => {
                for (_, e) in a {
                    self.set_mutability(e, mutable);
                }
            }
            Value::Macro(_) => (),
            _ => (),
        };
    }

    pub fn get_lifetime(&self, index: usize) -> u16 {
        self.map.get(&index).unwrap().lifetime
    }

    pub fn increment_lifetimes(&mut self) {
        for (_, val) in self.map.iter_mut() {
            (*val).lifetime += 1;
        }
    }

    pub fn decrement_lifetimes(&mut self) {
        for (_, val) in self.map.iter_mut() {
            (*val).lifetime -= 1;
        }
    }

    pub fn clean_up(&mut self) {
        let mut to_be_removed = Vec::new();
        for (index, val) in self.map.iter() {
            if val.lifetime == 0 {
                to_be_removed.push(*index);
                //println!("removing value: {:?}", val.0);
            }
        }
        for index in to_be_removed {
            self.map.remove(&index);
        }
    }

    pub fn increment_single_lifetime(
        &mut self,
        index: usize,
        amount: u16,
        already_done: &mut HashSet<usize>,
    ) {
        if already_done.get(&index) == None {
            (*already_done).insert(index);
        } else {
            return;
        }
        let val = &mut (*self
            .map
            .get_mut(&index)
            .expect(&(index.to_string() + " index not found")))
        .lifetime;

        if *val < 10000 - amount {
            *val += amount;
        }

        match self[index].clone() {
            Value::Array(a) => {
                for e in a {
                    self.increment_single_lifetime(e, amount, already_done)
                }
            }
            Value::Dict(a) => {
                for (_, e) in a {
                    self.increment_single_lifetime(e, amount, already_done)
                }
            }
            Value::Macro(m) => {
                for (_, e, _, e2) in m.args {
                    if let Some(val) = e {
                        self.increment_single_lifetime(val, amount, already_done)
                    }
                    if let Some(val) = e2 {
                        self.increment_single_lifetime(val, amount, already_done)
                    }
                }

                for (_, v) in m.def_context.variables.iter() {
                    self.increment_single_lifetime(*v, amount, already_done)
                }
            }
            _ => (),
        };
    }
}

pub fn store_value(
    val: Value,
    lifetime: u16,
    globals: &mut Globals,
    context: &Context,
) -> StoredValue {
    let index = globals.val_id;
    let mutable = !matches!(val, Value::Macro(_));

    (*globals).stored_values.map.insert(
        index,
        StoredValData {
            val,
            fn_context: context.start_group,
            mutable,
            lifetime,
        },
    );
    (*globals).val_id += 1;
    index
}
pub fn clone_and_get_value(
    index: usize,
    lifetime: u16,
    globals: &mut Globals,
    fn_context: Group,
    constant: bool,
) -> Value {
    let mut old_val = globals.stored_values[index].clone();

    match &mut old_val {
        Value::Array(arr) => {
            old_val = Value::Array(
                arr.iter()
                    .map(|x| clone_value(*x, lifetime, globals, fn_context, constant))
                    .collect(),
            );
        }

        Value::Dict(arr) => {
            old_val = Value::Dict(
                arr.iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            clone_value(*v, lifetime, globals, fn_context, constant),
                        )
                    })
                    .collect(),
            );
        }

        Value::Macro(m) => {
            for arg in &mut m.args {
                if let Some(def_val) = &mut arg.1 {
                    (*def_val) = clone_value(*def_val, lifetime, globals, fn_context, constant);
                }

                if let Some(def_val) = &mut arg.3 {
                    (*def_val) = clone_value(*def_val, lifetime, globals, fn_context, constant);
                }
            }

            // for (_, v) in m.def_context.variables.iter_mut() {
            //     (*v) = clone_value(*v, lifetime, globals, context, constant)
            // }
        }
        _ => (),
    };

    old_val
}

pub fn clone_value(
    index: usize,
    lifetime: u16,
    globals: &mut Globals,
    fn_context: Group,
    constant: bool,
) -> StoredValue {
    let old_val = clone_and_get_value(index, lifetime, globals, fn_context, constant);

    //clone all inner values
    //do the thing
    //bing bang
    //profit
    let new_index = globals.val_id;

    (*globals).stored_values.map.insert(
        new_index,
        StoredValData {
            val: old_val,
            fn_context,
            mutable: !constant,
            lifetime,
        },
    );
    (*globals).val_id += 1;
    new_index
}

pub fn store_const_value(
    val: Value,
    lifetime: u16,
    globals: &mut Globals,
    context: &Context,
) -> StoredValue {
    let index = globals.val_id;

    (*globals).stored_values.map.insert(
        index,
        StoredValData {
            val,
            fn_context: context.start_group,
            mutable: false,
            lifetime,
        },
    );
    (*globals).val_id += 1;
    index
}

pub fn store_val_m(
    val: Value,
    lifetime: u16,
    globals: &mut Globals,
    context: &Context,
    constant: bool,
) -> StoredValue {
    let index = globals.val_id;

    (*globals).stored_values.map.insert(
        index,
        StoredValData {
            val,
            fn_context: context.start_group,
            mutable: !constant,
            lifetime,
        },
    );
    (*globals).val_id += 1;
    index
}

//! The evaluator — YOUR file.  \[student\]
//!
//! [`eval_expr`](Interpreter::eval_expr) turns one [`Expr`] into a [`Value`], or
//! leaves evaluation early through [`Control`] (a stuck `Raise`, or a `Return`).
//! It is the heart of the interpreter and the one method you grow across the
//! milestones. Every `Expr` variant already has an arm; a variant you have not
//! reached yet is a milestone-tagged hole. To list a milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m3!' src
//! ```
//!
//! To fill one in, replace its `todo_mN!(..)` with the rule's implementation,
//! destructuring the arm's `..` into the fields you need — for example
//! `Expr::Lit(lit, span) => …`. A block is just `Expr::Block`, so statement and
//! block evaluation live inside this one method; factor out a helper if you
//! like — the grader only ever calls `eval_expr`.
//!
//! Two contracts these arms must keep. A Bridger call — a function or method
//! body, and the `from` a `?` runs — is entered through
//! `Interpreter::enter_call` and left through `Interpreter::leave_call`,
//! balanced, so runaway recursion becomes a clean `RuntimeError::StackOverflow`
//! rather than aborting the process; this is the run-time call depth, separate
//! from the static nesting bound the checker enforces. And the `?` and
//! method-dispatch arms (M7, M8) read the type checker's choices from
//! `self.conversions`, keyed by an expression's span: look one up at the same
//! span the checker recorded it at (`Checker::record_conversion`), or a `?`
//! quietly passes its payload through unconverted.

use super::Interpreter;
use super::{Control, Env, Value};
use crate::ast::Expr;

impl Interpreter {
    /// Evaluate `e` in environment `env`.
    pub fn eval_expr(&mut self, e: &Expr, env: &Env) -> Result<Value, Control> {
        // `env` goes unused until M2 (E-Var, E-Block). Delete this line once you
        // read from `env`; it only keeps the unused-variable lint quiet for now.
        let _ = env;
        match e {
            // ---- M1: expressions ----
            Expr::Lit(lit, _span) => {
                let v = match lit {
                    crate::ast::Lit::Int(i) => Value::Int(*i),
                    crate::ast::Lit::Bool(b) => Value::Bool(*b),
                    crate::ast::Lit::Str(s) => Value::Str(s.clone().into()),
                    crate::ast::Lit::Unit => Value::Unit,
                };
                Ok(v)
            }
            Expr::Unary(op, sub, span) => {
                let v = self.eval_expr(sub, env)?;
                match (op, v) {
                    (crate::ast::UnOp::Neg, Value::Int(i)) => Ok(Value::Int(i.wrapping_neg())),
                    (crate::ast::UnOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    (crate::ast::UnOp::Neg, other) => {
                        Err(crate::interp::error::RuntimeError::TypeError {
                            expected: crate::ast::Ty::int(),
                            found: crate::interp::value::type_of(&other),
                            span: *span,
                        }
                        .into())
                    }
                    (crate::ast::UnOp::Not, other) => {
                        Err(crate::interp::error::RuntimeError::TypeError {
                            expected: crate::ast::Ty::bool(),
                            found: crate::interp::value::type_of(&other),
                            span: *span,
                        }
                        .into())
                    }
                    (crate::ast::UnOp::Ref, _) | (crate::ast::UnOp::Deref, _) => {
                        todo_m3!("E-Ref / E-Deref")
                    }
                }
            }
            Expr::Binary(op, lhs, rhs, span) => {
                use crate::ast::BinOp::*;
                use crate::ast::Ty;
                use crate::interp::error::RuntimeError;
                use crate::interp::value::{type_of, List};

                if matches!(op, And) {
                    return match self.eval_expr(lhs, env)? {
                        Value::Bool(false) => Ok(Value::Bool(false)),
                        Value::Bool(true) => match self.eval_expr(rhs, env)? {
                            Value::Bool(b) => Ok(Value::Bool(b)),
                            other => Err(RuntimeError::TypeError {
                                expected: Ty::bool(),
                                found: type_of(&other),
                                span: *span,
                            }
                            .into()),
                        },
                        other => Err(RuntimeError::TypeError {
                            expected: Ty::bool(),
                            found: type_of(&other),
                            span: *span,
                        }
                        .into()),
                    };
                }
                if matches!(op, Or) {
                    return match self.eval_expr(lhs, env)? {
                        Value::Bool(true) => Ok(Value::Bool(true)),
                        Value::Bool(false) => match self.eval_expr(rhs, env)? {
                            Value::Bool(b) => Ok(Value::Bool(b)),
                            other => Err(RuntimeError::TypeError {
                                expected: Ty::bool(),
                                found: type_of(&other),
                                span: *span,
                            }
                            .into()),
                        },
                        other => Err(RuntimeError::TypeError {
                            expected: Ty::bool(),
                            found: type_of(&other),
                            span: *span,
                        }
                        .into()),
                    };
                }

                let l = self.eval_expr(lhs, env)?;
                let r = self.eval_expr(rhs, env)?;

                match op {
                    Eq => Ok(Value::Bool(l == r)),
                    Ne => Ok(Value::Bool(l != r)),

                    Add | Sub | Mul | Div | Mod | Lt | Le | Gt | Ge => {
                        let (a, b) = match (&l, &r) {
                            (Value::Int(a), Value::Int(b)) => (*a, *b),
                            (Value::Int(_), _) => {
                                return Err(RuntimeError::TypeError {
                                    expected: Ty::int(),
                                    found: type_of(&r),
                                    span: *span,
                                }
                                .into())
                            }
                            _ => {
                                return Err(RuntimeError::TypeError {
                                    expected: Ty::int(),
                                    found: type_of(&l),
                                    span: *span,
                                }
                                .into())
                            }
                        };
                        match op {
                            Add => Ok(Value::Int(a.wrapping_add(b))),
                            Sub => Ok(Value::Int(a.wrapping_sub(b))),
                            Mul => Ok(Value::Int(a.wrapping_mul(b))),
                            Div if b == 0 => Err(RuntimeError::DivByZero { span: *span }.into()),
                            Div => Ok(Value::Int(a.wrapping_div(b))),
                            Mod if b == 0 => Err(RuntimeError::DivByZero { span: *span }.into()),
                            Mod => Ok(Value::Int(a.wrapping_rem(b))),
                            Lt => Ok(Value::Bool(a < b)),
                            Le => Ok(Value::Bool(a <= b)),
                            Gt => Ok(Value::Bool(a > b)),
                            Ge => Ok(Value::Bool(a >= b)),
                            _ => unreachable!(),
                        }
                    }

                    Concat => match (l, r) {
                        (Value::Str(a), Value::Str(b)) => {
                            Ok(Value::Str(std::rc::Rc::from(format!("{a}{b}"))))
                        }
                        (Value::List(a), Value::List(b)) => Ok(Value::List(a.concat(&b))),
                        (bad_l, _bad_r) => Err(RuntimeError::TypeError {
                            expected: type_of(&bad_l),
                            found: type_of(&bad_l), 
                            span: *span,
                        }
                        .into()),
                    },

                    Cons => match r {
                        Value::List(tail) => Ok(Value::List(List::cons(l, tail))),
                        other => Err(RuntimeError::TypeError {
                            expected: Ty::list(type_of(&l)),
                            found: type_of(&other),
                            span: *span,
                        }
                        .into()),
                    },

                    And | Or => unreachable!("handled above"),
                }
            }
            Expr::Tuple(elems, _span) => {
                let mut values = Vec::with_capacity(elems.len());
                for elem in elems {
                    values.push(self.eval_expr(elem, env)?);
                }
                Ok(Value::Tuple(values.into()))
            }
            Expr::List(elems, _span) => {
                let mut values = Vec::with_capacity(elems.len());
                for elem in elems {
                    values.push(self.eval_expr(elem, env)?);
                }
                Ok(Value::List(values.into()))
            }
            Expr::Proj(tuple_expr, idx, span) => match self.eval_expr(tuple_expr, env)? {
                Value::Tuple(items) => match items.get(*idx as usize) {
                    Some(v) => Ok(v.clone()),
                    None => Err(crate::interp::error::RuntimeError::NoSuchField {
                        field: idx.to_string(),
                        span: *span,
                    }
                    .into()),
                },
                _ => Err(crate::interp::error::RuntimeError::NoSuchField {
                    field: idx.to_string(),
                    span: *span,
                }
                .into()),
            },

            // ---- M2: binding ----
            Expr::Var(..) => todo_m2!("E-Var"),
            Expr::Block(..) => todo_m2!("E-Block / E-Let / E-Seq"),

            // ---- M3: state & control ----
            Expr::If(..) => todo_m3!("E-If"),
            Expr::While(..) => todo_m3!("E-While"),
            Expr::For(..) => todo_m3!("E-For"),
            Expr::Assign(..) => todo_m3!("E-Assign"),
            Expr::Return(..) => todo_m3!("E-Return"),

            // ---- M4: functions ----
            Expr::Lambda(..) => todo_m4!("E-Lam"),
            Expr::Call(..) => todo_m4!("E-App (dispatch Bridger vs. Native closure)"),

            // ---- M6: relations (queries go through `self.solutions`, engine.rs) ----
            Expr::Relation(..) => todo_m6!("E-Add / E-Clear / E-Solutions / E-Query"),
            Expr::ForQuery(..) => todo_m6!("E-ForQuery (iterate a query's solutions)"),

            // ---- M7: algebraic data types ----
            Expr::Ctor(..) => todo_m7!("E-Ctor"),
            Expr::Match(..) => todo_m7!("E-Match"),
            Expr::Try(..) => todo_m7!("E-Try (`?`: match + return)"),

            // ---- M8: objects ----
            Expr::Struct(..) => todo_m8!("E-Struct"),
            Expr::Field(..) => todo_m8!("E-Field (struct field access)"),
            Expr::Method(..) => todo_m8!("E-Method (head-type dispatch)"),
        }
    }
}

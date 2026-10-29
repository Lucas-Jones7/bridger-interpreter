//! The type checker — YOUR file.  \[student\]  (Milestone M5, growing after)
//!
//! [`check_expr`](Checker::check_expr) is to the typing rules what `eval_expr`
//! is to the evaluation rules: one arm per rule, over the same `Expr`. It is
//! **bidirectional**. With an `expected` type the arm *checks* `e` against it
//! (the mode a lambda needs, where the parameter types come from the expected
//! function type); with `None` it *infers* the type from `e` alone. Either way
//! it returns `e`'s type and records it with [`Checker::record`].
//!
//! Every arm is a milestone-tagged hole. The checker arrives at M5 with the
//! expression language of M1–M4; each later milestone's forms get their typing
//! rules when the milestone lands. To list a milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m5!' src
//! ```
//!
//! Where a rule needs two types to agree, call [`Checker::unify`] rather than
//! comparing with `==`: an unannotated `let`, a lambda parameter, an empty
//! list, or `deref` of a not-yet-known reference gets a
//! [`TyKind::Meta`](crate::ast::TyKind::Meta), and the same `unify` call that
//! compares two known types solves it.
//!
//! A few choices the checker makes cannot be recovered at run time, so it hands
//! them to the evaluator through `Checker::record_conversion`, keyed by an
//! expression's span: the `from` a `?` converts through when the error types
//! differ, and the impl chosen when a method call's receiver leaves it
//! ambiguous (both M8). Record at the same span the evaluator looks up, or the
//! choice is silently lost.

use super::{Checker, Ctx, TyError, MAX_NESTING};
use crate::ast::{
    BinOp, Decl, Expr, Generics, Lit, Name, Param, Rule, Span, Stmt, TraitRef, Ty, TyKind, UnOp,
};
use std::collections::HashMap;

impl Checker {
    /// The type of `e` in context `ctx`, checked against `expected` when one
    /// is given. Records the result at `e`'s span.
    pub fn check_expr(
        &mut self,
        e: &Expr,
        ctx: &Ctx,
        expected: Option<&Ty>,
    ) -> Result<Ty, TyError> {
        // `ctx` and `expected` are read by the arms you write; this line only
        // keeps the unused-variable lint quiet until then.
        if self.depth >= MAX_NESTING {
            return Err(TyError::TooDeep {
                limit: MAX_NESTING,
                span: e.span(),
            });
        }
        self.depth += 1;
        let r = self.check_inner(e, ctx, expected);
        self.depth -= 1;
        r
    }

    fn finish(&mut self, ty: Ty, expected: Option<&Ty>, span: Span) -> Result<Ty, TyError> {
        if let Some(want) = expected {
            self.unify(&ty, want, span)?;
        }
        let ty = self.resolve(&ty);
        self.record(span, ty.clone());
        Ok(ty)
    }

    fn instantiate(&mut self, generics: &Generics, params: &[Param], ret: &Ty, span: Span) -> Ty {
        let mut map: HashMap<Name, Ty> = HashMap::new();
        for p in &generics.params {
            let m = self.fresh_meta();
            map.insert(p.name.clone(), m);
        }
        for p in &generics.params {
            if let Some(b) = &p.bound {
                let tr = TraitRef {
                    name: b.name.clone(),
                    args: b.args.iter().map(|a| a.substitute(&map)).collect(),
                    span,
                };
                self.require_trait(tr, map[&p.name].clone(), span);
            }
        }
        let ps = params.iter().map(|p| p.ty.substitute(&map)).collect();
        Ty::func(ps, ret.substitute(&map))
    }

    fn check_inner(&mut self, e: &Expr, ctx: &Ctx, expected: Option<&Ty>) -> Result<Ty, TyError> {
        match e {
            // ---- M5: the checker over M1–M4's expressions ----
            Expr::Lit(lit, span) => {
                let ty = match lit {
                    Lit::Int(_) => Ty::int(),
                    Lit::Bool(_) => Ty::bool(),
                    Lit::Str(_) => Ty::str(),
                    Lit::Unit => Ty::unit(),
                };
                self.finish(ty, expected, *span)
            }
            Expr::Var(x, span) => {
                let ty = if let Some(t) = ctx.lookup(x) {
                    t
                } else if let Some(r) = self.global_type(x) {
                    r?
                } else if let Some(Decl::Fn(g, ps, ret, _)) = self.decl(x).cloned() {
                    self.instantiate(&g, &ps, &ret, *span)
                } else if let Some(t) = self.native_fn_type(x, *span) {
                    t
                } else {
                    return Err(TyError::UnboundVariable {
                        name: x.clone(),
                        span: *span,
                    });
                };
                self.finish(ty, expected, *span)
            }
            Expr::Unary(op, sub, span) => match op {
                UnOp::Neg => {
                    self.check_expr(sub, ctx, Some(&Ty::int()))?;
                    self.finish(Ty::int(), expected, *span)
                }
                UnOp::Not => {
                    self.check_expr(sub, ctx, Some(&Ty::bool()))?;
                    self.finish(Ty::bool(), expected, *span)
                }
                UnOp::Ref => {
                    let want = expected.map(|t| self.resolve(t));
                    let inner = match want.as_ref().map(|t| &t.kind) {
                        Some(TyKind::Ref(t)) => Some((**t).clone()),
                        _ => None,
                    };
                    let t = self.check_expr(sub, ctx, inner.as_ref())?;
                    self.finish(Ty::reference(t), expected, *span)
                }
                UnOp::Deref => {
                    let m = self.fresh_meta();
                    self.check_expr(sub, ctx, Some(&Ty::reference(m.clone())))?;
                    self.finish(m, expected, *span)
                }
            },
            Expr::Binary(op, lhs, rhs, span) => match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    self.check_expr(lhs, ctx, Some(&Ty::int()))?;
                    self.check_expr(rhs, ctx, Some(&Ty::int()))?;
                    self.finish(Ty::int(), expected, *span)
                }
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    self.check_expr(lhs, ctx, Some(&Ty::int()))?;
                    self.check_expr(rhs, ctx, Some(&Ty::int()))?;
                    self.finish(Ty::bool(), expected, *span)
                }
                BinOp::And | BinOp::Or => {
                    self.check_expr(lhs, ctx, Some(&Ty::bool()))?;
                    self.check_expr(rhs, ctx, Some(&Ty::bool()))?;
                    self.finish(Ty::bool(), expected, *span)
                }
                BinOp::Eq | BinOp::Ne => {
                    let l = self.check_expr(lhs, ctx, None)?;
                    self.check_expr(rhs, ctx, Some(&l))?;
                    self.require_bound("Eq".to_string(), l, *span);
                    self.finish(Ty::bool(), expected, *span)
                }
                BinOp::Concat => {
                    let l = self.check_expr(lhs, ctx, None)?;
                    let l = self.resolve(&l);
                    let ty = match &l.kind {
                        TyKind::Str => {
                            self.check_expr(rhs, ctx, Some(&Ty::str()))?;
                            l.clone()
                        }
                        TyKind::List(_) => {
                            self.check_expr(rhs, ctx, Some(&l))?;
                            l.clone()
                        }
                        TyKind::Meta(_) => {
                            let m = self.undetermined(lhs.span())?;
                            self.check_expr(rhs, ctx, None)?;
                            m
                        }
                        _ => {
                            self.unify(&l, &Ty::str(), lhs.span())?;
                            Ty::str()
                        }
                    };
                    self.finish(ty, expected, *span)
                }
                BinOp::Cons => {
                    let l = self.check_expr(lhs, ctx, None)?;
                    self.check_expr(rhs, ctx, Some(&Ty::list(l.clone())))?;
                    self.finish(Ty::list(l), expected, *span)
                }
            },
            Expr::Tuple(es, span) => {
                let want = expected.map(|t| self.resolve(t));
                let mut tys = Vec::with_capacity(es.len());
                for (i, x) in es.iter().enumerate() {
                    let exp_i = match want.as_ref().map(|t| &t.kind) {
                        Some(TyKind::Tuple(ts)) if ts.len() == es.len() => Some(ts[i].clone()),
                        _ => None,
                    };
                    tys.push(self.check_expr(x, ctx, exp_i.as_ref())?);
                }
                self.finish(Ty::tuple(tys), expected, *span)
            }
            Expr::List(es, span) => {
                let want = expected.map(|t| self.resolve(t));
                let elem = match want.as_ref().map(|t| &t.kind) {
                    Some(TyKind::List(t)) => (**t).clone(),
                    _ => self.fresh_meta(),
                };
                for x in es {
                    self.check_expr(x, ctx, Some(&elem))?;
                }
                self.finish(Ty::list(elem), expected, *span)
            }
            Expr::Proj(sub, idx, span) => {
                let t = self.check_expr(sub, ctx, None)?;
                let t = self.resolve(&t);
                let ty = match &t.kind {
                    TyKind::Tuple(ts) => match ts.get(*idx as usize) {
                        Some(c) => c.clone(),
                        None => {
                            return Err(TyError::NoSuchComponent {
                                index: *idx,
                                arity: ts.len(),
                                span: *span,
                            })
                        }
                    },
                    TyKind::Meta(_) => self.undetermined(sub.span())?,
                    _ => {
                        return Err(TyError::NotATuple {
                            index: *idx,
                            found: t.clone(),
                            span: *span,
                        })
                    }
                };
                self.finish(ty, expected, *span)
            }
            Expr::Block(stmts, tail, span) => {
                let mut ctx = ctx.clone();
                for s in stmts {
                    let sub = match s {
                        Stmt::Let(name, ann, rhs, let_span) => {
                            let declared = match ann {
                                Some(t) => {
                                    Some(self.annotation(t, &ctx, t.span.unwrap_or(*let_span))?)
                                }
                                None => None,
                            };
                            let t = self.check_expr(rhs, &ctx, declared.as_ref())?;
                            ctx = ctx.extend(name.clone(), declared.unwrap_or(t));
                            rhs.span()
                        }
                        Stmt::Expr(x) => {
                            self.check_expr(x, &ctx, None)?;
                            x.span()
                        }
                    };
                    if self.is_diverging(sub) {
                        self.dead_after(sub, *span);
                    }
                }
                let ty = match tail {
                    Some(t) => {
                        let ty = self.check_expr(t, &ctx, expected)?;
                        if self.is_diverging(t.span()) {
                            self.record_diverging(*span);
                        }
                        ty
                    }
                    None if self.is_diverging(*span) => self.fresh_meta(),
                    None => Ty::unit(),
                };
                self.finish(ty, expected, *span)
            }
            Expr::If(cond, then, els, span) => {
                self.check_expr(cond, ctx, Some(&Ty::bool()))?;
                if self.is_diverging(cond.span()) {
                    self.dead_after(cond.span(), *span);
                }
                match els {
                    Some(els) => {
                        let target = match expected {
                            Some(t) => t.clone(),
                            None => self.check_expr(then, ctx, None)?,
                        };
                        if expected.is_some() {
                            self.check_expr(then, ctx, Some(&target))?;
                        }
                        self.check_expr(els, ctx, Some(&target))?;
                        if self.is_diverging(then.span()) && self.is_diverging(els.span()) {
                            self.record_diverging(*span);
                        }
                        self.finish(target, expected, *span)
                    }
                    None => {
                        let t = self.check_expr(then, ctx, None)?;
                        let t = self.resolve(&t);
                        if !self.is_diverging(then.span()) {
                            match &t.kind {
                                TyKind::Meta(_) => self.unify(&t, &Ty::unit(), then.span())?,
                                TyKind::Unit => {}
                                _ => {
                                    return Err(TyError::IfWithoutElse {
                                        found: t,
                                        span: *span,
                                    })
                                }
                            }
                        }
                        self.finish(Ty::unit(), expected, *span)
                    }
                }
            }
            Expr::While(cond, body, span) => {
                self.check_expr(cond, ctx, Some(&Ty::bool()))?;
                if self.is_diverging(cond.span()) {
                    self.dead_after(cond.span(), *span);
                }
                self.check_expr(body, ctx, Some(&Ty::unit()))?;
                self.finish(Ty::unit(), expected, *span)
            }
            Expr::For(x, iter, body, span) => {
                let m = self.fresh_meta();
                self.check_expr(iter, ctx, Some(&Ty::list(m.clone())))?;
                if self.is_diverging(iter.span()) {
                    self.dead_after(iter.span(), *span);
                }
                let inner = ctx.extend(x.clone(), m);
                self.check_expr(body, &inner, Some(&Ty::unit()))?;
                self.finish(Ty::unit(), expected, *span)
            }
            Expr::Assign(lhs, rhs, span) => {
                let m = self.fresh_meta();
                self.check_expr(lhs, ctx, Some(&Ty::reference(m.clone())))?;
                self.check_expr(rhs, ctx, Some(&m))?;
                self.finish(Ty::unit(), expected, *span)
            }
            Expr::Return(operand, span) => {
                let Some(ret) = ctx.ret() else {
                    return Err(TyError::ReturnOutsideFunction { span: *span });
                };
                self.check_expr(operand, ctx, Some(&ret))?;
                self.record_diverging(*span);
                let m = self.fresh_meta();
                self.finish(m, expected, *span)
            }
            Expr::Lambda(params, body, span) => {
                let want = expected.map(|t| self.resolve(t));
                let (exp_ps, exp_ret) = match want.as_ref().map(|t| &t.kind) {
                    Some(TyKind::Fn(ps, r)) if ps.len() == params.len() => {
                        (Some(ps.clone()), Some((**r).clone()))
                    }
                    _ => (None, None),
                };
                let mut ptys = Vec::with_capacity(params.len());
                for (i, p) in params.iter().enumerate() {
                    let w = exp_ps.as_ref().map(|ps| ps[i].clone());
                    let pty = match (&p.ty, w) {
                        (Some(a), Some(w)) => {
                            let a = self.annotation(a, ctx, p.span)?;
                            self.unify(&a, &w, p.span)?;
                            a
                        }
                        (Some(a), None) => self.annotation(a, ctx, p.span)?,
                        (None, Some(w)) => w,
                        (None, None) => self.fresh_meta(),
                    };
                    ptys.push(pty);
                }
                let ret = match exp_ret {
                    Some(r) => r,
                    None => self.fresh_meta(),
                };
                let mut inner = ctx.enter(Some(ret.clone()), ctx.self_ty());
                for (p, t) in params.iter().zip(&ptys) {
                    inner = inner.extend(p.name.clone(), t.clone());
                }
                self.check_expr(body, &inner, Some(&ret))?;
                self.finish(Ty::func(ptys, ret), expected, *span)
            }
            Expr::Call(f, args, span) => {
                let (fty, bounds) = self.check_callee(f, ctx)?;
                let name = match &**f {
                    Expr::Var(n, _) => n.clone(),
                    _ => "<expression>".to_string(),
                };
                let (ps, ret): (Vec<Ty>, Ty) = match &fty.kind {
                    TyKind::Fn(ps, r) => {
                        if ps.len() != args.len() {
                            return Err(TyError::ArityMismatch {
                                callee: name,
                                expected: ps.len(),
                                found: args.len(),
                                span: *span,
                            });
                        }
                        for (a, p) in args.iter().zip(ps.iter()) {
                            self.check_expr(a, ctx, Some(p))?;
                        }
                        (ps.to_vec(), (**r).clone())
                    }
                    TyKind::Meta(_) => {
                        let mut tys = Vec::with_capacity(args.len());
                        for a in args {
                            tys.push(self.check_expr(a, ctx, None)?);
                        }
                        let r = self.fresh_meta();
                        self.unify(&fty, &Ty::func(tys.clone(), r.clone()), *span)?;
                        (tys, r)
                    }
                    _ => {
                        return Err(TyError::NotAFunction {
                            found: fty.clone(),
                            span: f.span(),
                        })
                    }
                };
                self.finish_call(bounds, &ps, args, *span);
                self.finish(ret, expected, *span)
            }

            // ---- M6: relations ----
            Expr::Relation(..) => todo_m6!("T-Add / T-Clear / T-Solutions / T-Query"),
            Expr::ForQuery(..) => todo_m6!("T-ForQuery"),

            // ---- M7: algebraic data types ----
            Expr::Ctor(..) => todo_m7!("T-Con"),
            Expr::Match(..) => todo_m7!("T-Match (patterns, then exhaustiveness)"),
            Expr::Try(..) => todo_m7!("T-Try"),

            // ---- M8: objects ----
            Expr::Struct(..) => todo_m8!("T-Struct"),
            Expr::Field(..) => todo_m8!("T-Field"),
            Expr::Method(..) => todo_m8!("T-Method (dispatch on the receiver's type; bounds)"),
        }
    }

    /// Make `a` and `b` the same type, or fail with a `Mismatch` at `span`.
    /// While every type is known this is a structural comparison; once
    /// inference variables exist it also *solves* them (`Checker::solve`),
    /// with an occurs check so no type contains itself.
    pub fn unify(&mut self, a: &Ty, b: &Ty, span: Span) -> Result<(), TyError> {
        let a = self.resolve(a);
        let b = self.resolve(b);
        match (&a.kind, &b.kind) {
            (TyKind::Meta(x), TyKind::Meta(y)) if x == y => Ok(()),
            (TyKind::Meta(x), _) => self.bind(*x, &b, span),
            (_, TyKind::Meta(y)) => self.bind(*y, &a, span),
            (TyKind::Int, TyKind::Int)
            | (TyKind::Bool, TyKind::Bool)
            | (TyKind::Str, TyKind::Str)
            | (TyKind::Unit, TyKind::Unit) => Ok(()),
            (TyKind::Tuple(xs), TyKind::Tuple(ys)) if xs.len() == ys.len() => {
                for (x, y) in xs.iter().zip(ys.iter()) {
                    self.unify(x, y, span)?;
                }
                Ok(())
            }
            (TyKind::List(x), TyKind::List(y)) | (TyKind::Ref(x), TyKind::Ref(y)) => {
                self.unify(x, y, span)
            }
            (TyKind::Fn(ps, r), TyKind::Fn(qs, s)) if ps.len() == qs.len() => {
                for (x, y) in ps.iter().zip(qs.iter()) {
                    self.unify(x, y, span)?;
                }
                self.unify(r, s, span)
            }
            (TyKind::Named(n, xs), TyKind::Named(m, ys)) if n == m && xs.len() == ys.len() => {
                for (x, y) in xs.iter().zip(ys.iter()) {
                    self.unify(x, y, span)?;
                }
                Ok(())
            }
            _ => Err(TyError::Mismatch {
                expected: b,
                found: a,
                span,
            }),
        }
    }

    fn bind(&mut self, id: u32, ty: &Ty, span: Span) -> Result<(), TyError> {
        if occurs(id, ty) {
            return Err(TyError::InfiniteType { span });
        }
        self.solve(id, ty.clone());
        Ok(())
    }

    /// Type-check one rule (Appendix D, "Relations"): bind each generator's
    /// logic variables at their relation's column types, check the head's
    /// terms against the head relation's columns, and check every filter
    /// against `Bool`. A distinct judgment from `check_expr`, which it calls
    /// for the filters; the driver runs it over each rule once relations
    /// arrive.
    pub fn check_rule(&mut self, rule: &Rule) -> Result<(), TyError> {
        let _ = rule;
        todo_m6!("check_rule (generators bind logic variables; head and filters typed)")
    }
}

fn occurs(id: u32, ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Meta(m) => *m == id,
        TyKind::Tuple(ts) | TyKind::Named(_, ts) => ts.iter().any(|t| occurs(id, t)),
        TyKind::List(t) | TyKind::Ref(t) => occurs(id, t),
        TyKind::Fn(ps, r) => ps.iter().any(|t| occurs(id, t)) || occurs(id, r),
        _ => false,
    }
}

/// Provided helpers (Milestone M5): plumbing that no typing rule describes.
/// Paste this module at the end of the file, unchanged.
mod helpers {
    use crate::ast::{Decl, Expr, Name, Span, Ty, TyKind};
    use crate::types::{Checker, Ctx, TyError};
    use std::ops::Range;

    type Res<T> = Result<T, TyError>;
    type Bounds = Range<usize>;

    impl Checker {
        /// A type annotation written inside a body (`let x: T`, `|x: T|`): `Self`
        /// resolved to the enclosing `impl`'s type, and every name it uses checked
        /// to be a declared type or a type parameter in scope (`Self` outside an
        /// `impl` names no type).
        pub fn annotation(&self, ty: &Ty, ctx: &Ctx, span: Span) -> Res<Ty> {
            let ty = match ctx.self_ty() {
                Some(self_ty) => ty.with_self(&self_ty),
                None => ty.clone(),
            };
            self.validate_type(&ty, span)?;
            Ok(ty)
        }

        /// The type of a native prelude function (Appendix C, "Functions").
        /// `print` / `println` / `to_string` carry a `Print` bound and `contains`
        /// an `Eq` bound; until M8 `len` carries `Len` and `min` / `max` /
        /// `minimum` / `maximum` carry `Ord`, which the built-in types satisfy.
        /// `None` for a name that is not a native primitive.
        pub fn native_fn_type(&mut self, name: &str, span: Span) -> Option<Ty> {
            let opt = |t: Ty| Ty::named("Option".to_string(), vec![t]);
            let sig = match name {
                "print" | "println" => {
                    let t = self.fresh_meta();
                    self.require_bound("Print".to_string(), t.clone(), span);
                    Ty::func(vec![t], Ty::unit())
                }
                "to_string" => {
                    let t = self.fresh_meta();
                    self.require_bound("Print".to_string(), t.clone(), span);
                    Ty::func(vec![t], Ty::str())
                }
                "abs" => Ty::func(vec![Ty::int()], Ty::int()),
                "even" | "odd" => Ty::func(vec![Ty::int()], Ty::bool()),
                "range" => Ty::func(vec![Ty::int(), Ty::int()], Ty::list(Ty::int())),
                #[cfg(not(feature = "m8"))]
                "len" => {
                    let t = self.fresh_meta();
                    self.require_bound("Len".to_string(), t.clone(), span);
                    Ty::func(vec![t], Ty::int())
                }
                #[cfg(not(feature = "m8"))]
                "min" | "max" => {
                    let t = self.fresh_meta();
                    self.require_bound("Ord".to_string(), t.clone(), span);
                    Ty::func(vec![t.clone(), t.clone()], t)
                }
                #[cfg(not(feature = "m8"))]
                "minimum" | "maximum" => {
                    let t = self.fresh_meta();
                    self.require_bound("Ord".to_string(), t.clone(), span);
                    Ty::func(vec![Ty::list(t.clone())], opt(t))
                }
                "contains" => {
                    let t = self.fresh_meta();
                    self.require_bound("Eq".to_string(), t.clone(), span);
                    Ty::func(vec![Ty::list(t.clone()), t], Ty::bool())
                }
                "head" => {
                    let t = self.fresh_meta();
                    Ty::func(vec![Ty::list(t.clone())], opt(t))
                }
                "tail" => {
                    let t = self.fresh_meta();
                    Ty::func(vec![Ty::list(t.clone())], opt(Ty::list(t)))
                }
                "unwrap_or" => {
                    let t = self.fresh_meta();
                    Ty::func(vec![opt(t.clone()), t.clone()], t)
                }
                "map" => {
                    let (a, b) = (self.fresh_meta(), self.fresh_meta());
                    Ty::func(
                        vec![Ty::list(a.clone()), Ty::func(vec![a], b.clone())],
                        Ty::list(b),
                    )
                }
                "filter" => {
                    let a = self.fresh_meta();
                    let keep = Ty::func(vec![a.clone()], Ty::bool());
                    Ty::func(vec![Ty::list(a.clone()), keep], Ty::list(a))
                }
                "fold" => {
                    let (a, b) = (self.fresh_meta(), self.fresh_meta());
                    Ty::func(
                        vec![
                            Ty::list(a.clone()),
                            b.clone(),
                            Ty::func(vec![b.clone(), a], b.clone()),
                        ],
                        b,
                    )
                }
                "is_some" => {
                    let t = self.fresh_meta();
                    Ty::func(vec![opt(t)], Ty::bool())
                }
                "read_int" => Ty::func(vec![], Ty::int()),
                "read_bool" => Ty::func(vec![], Ty::bool()),
                "read_line" => Ty::func(vec![], Ty::str()),
                "read_int_opt" => Ty::func(vec![], opt(Ty::int())),
                "read_bool_opt" => Ty::func(vec![], opt(Ty::bool())),
                "read_line_opt" => Ty::func(vec![], opt(Ty::str())),
                _ => return None,
            };
            Some(sig)
        }

        /// The type of the top-level global `x`: its annotation, or the type
        /// the driver recorded for its initializer. `None` if `x` is not a
        /// global.
        pub fn global_type(&mut self, x: &Name) -> Option<Res<Ty>> {
            let (annot, init) = match self.decl(x) {
                Some(Decl::Global(annot, init)) => (annot.clone(), init.clone()),
                _ => return None,
            };
            Some(match (annot, self.global_types.get(x)) {
                (Some(ty), _) => Ok(ty),
                (None, Some(ty)) => Ok(ty.clone()),
                (None, None) => self.check_expr(&init, &Ctx::new(None, None), None),
            })
        }

        /// Synthesize the type of a call's callee, resolved, and note what the
        /// call's bookkeeping needs: a named callee is exempt from the
        /// determination rule, and the bounds the callee's type carries come
        /// back as a range to pass to `finish_call`.
        pub fn check_callee(&mut self, f: &Expr, ctx: &Ctx) -> Res<(Ty, Bounds)> {
            let start = self.obligations.len();
            let ty = self.check_expr(f, ctx, None)?;
            if matches!(f, Expr::Var(..)) {
                self.note_callee(f.span());
            }
            Ok((self.resolve(&ty), start..self.obligations.len()))
        }

        /// Call after checking a call's arguments against the parameter types
        /// `param_tys`: a bound the callee placed on a type parameter is
        /// reported at the argument whose parameter mentions it, or at the
        /// whole call if none does.
        pub fn finish_call(
            &mut self,
            bounds: Bounds,
            param_tys: &[Ty],
            args: &[Expr],
            call_span: Span,
        ) {
            for i in bounds {
                let want = &self.obligations[i].1;
                let blamed = param_tys.iter().position(|p| contains_ty(p, want));
                self.obligations[i].2 = match blamed {
                    Some(j) => args[j].span(),
                    None => call_span,
                };
            }
        }

        /// An operand whose type is still unknown where its rule needs it: in
        /// code that never runs nothing is required of it; elsewhere its type
        /// is undetermined.
        pub fn undetermined(&mut self, span: Span) -> Res<Ty> {
            if self.in_dead_code(span) {
                Ok(self.fresh_meta())
            } else {
                Err(TyError::Ambiguous { span })
            }
        }
    }

    /// Whether `needle` occurs in `hay` (structurally, spans aside).
    fn contains_ty(hay: &Ty, needle: &Ty) -> bool {
        if hay == needle {
            return true;
        }
        let any = |ts: &[Ty]| ts.iter().any(|t| contains_ty(t, needle));
        match &hay.kind {
            TyKind::Tuple(ts) | TyKind::Named(_, ts) => any(ts),
            TyKind::List(t) | TyKind::Ref(t) => contains_ty(t, needle),
            TyKind::Fn(ps, r) => any(ps) || contains_ty(r, needle),
            _ => false,
        }
    }
}

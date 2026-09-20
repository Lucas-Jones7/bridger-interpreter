# M1 Concept Questions
1. eval_expr returns Result<Value, Control>. Ok(v) means the expression means the expression returned a value. A stuck expression is when it cant further evaluate a step because it violates a runtime invariant the checker didnt catch. Stuck expressions come back as Err(Control) where the underlying error is a RuntimeError variant converted via .into(). For a type error, the RuntimeError::TypeError contains the type that was expected and the type that was actually found, along with the span of the operation that caused the error.

2. in Expr::Unary let v = self.ecal_xpr(sub, env)?; if evaluating sub gets stuck, it returns Err(Control::...), and ? immediately returns that Err from the outermost call, the unary arm never reaches its match (op, v). Without a ? it would look like:
    let v = match self.ecal_expr(sub, env) {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

3. In my and / or implementation it evaluates lhs first and uses a match to decide wheather the rhs needs to be evauluated. For and, the right side is only evaluated whent he left side is true. For or the right side is only evaluated when the left side is false. The rhs self.eval.expr call is inside the appropriate match branch, so it is skipped when the left side already determines th result.

4. Add, Sub, Mul, Div, and Mod use wrapping_add, wrapping_sub, and so on. Using wrapping methods makes the overflow a defined behavior instead of a lanugage panic that would crash the interpreter. The wrapping behavior is explicityly requested by using theses wrapping methods,so it doesnt depend on compiler settings.

5. every stuck expression uses span: *span, where span belongs to the enclosing Expr node being evaluated. the Expr::Binary arm uses its own span when a type error occurs. This is useful because the span points to the operation that caused the runtime error instead of pointing to a larger enclosing expression or only to a sub expression

# How I implemented it

1. In my Expr::Binary arm and / or are peeled off first and handled with short-circuiting match / return, since after this evaluation is safe. For all other operators both operators are evaluated: let l = self.eval_expr(lhs, env)?; let r = self.eval_expr(rhs, env)?; using ? twice here so that a stuck lhs shor circuits before the rhs is even looked at. Eq/Ne just compare the values with == and !=. The int only group does one match (&l, &r) to pull out ints or bail with a type error for whichever side want an int, then a second match op dispatches to the wrapping arithmetic with Div/Mod checking if b == 0 first before returning. Concat matches (l, r) structurally, Str + str builds a new Rc<str>, List + List calls .concat() and anything else is a type error. Cons checks that the right operand is a list and if it is it calls List::cons(l, tail) to put the left value at the front, otherwise a type error will be reported. Also every error path ends in into() to lift a runtime error into whatever COntrol wraps it, the ? or return gets it out of eval_expr.

2. The two trickest bits for me where the and / or short circuiting because my initial mistake was evaluating both sides before checking, as well as forgetting to check if a value was zero before the wrapping_div or wrapping_rem call.
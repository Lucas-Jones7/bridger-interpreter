# M1 Concept Questions ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
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



# M2 Concept Questions ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
What does env.lookup(x) return, and how does your Expr::Var arm turn each of its two outcomes into a result of eval_expr? For the stuck outcome, name the RuntimeError variant you build, which fields it carries, and which node's span it blames.

1. env.lookup(x) returns Option<Value>. The two match arms are Some and None, some returns the value, while none returns an UnboundVariable error. For the stuck outcome, the variant I build is UnboundVariable, its fields are name and span, and it blames the Var expression's own span from Expr::Var(name, span) not some other node's span.

Explain how your block arm threads the environment through its items. When you evaluate a Stmt::Let, in which environment do you evaluate its right-hand side, and what do you pass to the items that follow? Point to the lines where you call env.extend.

2. My block arm threads the environment through its items when it hits Stmt::Let(name, _ty, rhs, _), it calls self.eval_expr(rhs, &env) which is the environment as it stood before the let. Then env = env.extend(name.clone(), v) is where i call env.extend (line 238). The environment th enext iteration of the loop sees is the extended one.

env.extend returns a new Env and leaves the one it was called on untouched. Using that fact, explain why a name bound by a let inside a block is not in scope outside the block — refer to what happens to the block's environment when your arm returns.

3. The env variable inside the block arm is a local variable of that function call. extend builds a new environment and returns it, the env passed in as the &env parameter is never mutated. So when the blocks eval_expr call returns, that local env binding is dropped. The blocks own env variable goes out of scope when the function returns

In { let x = 1; let x = x + 1; x } the second let shadows the first rather than changing it. Explain, in terms of what extend does, why this produces 2 and not a mutation of the original binding — and why M2's let could not build a counter whose value changes over time.

4. This produces 2 rather than a mutation of the original binding because the second let's rhs looks up x in the environment as it exists before the second let binds, where x is still bound to 1. The result of that becomes a new fram layered on top of that instead of overwriting the old frame in place. M2's let cant build a counter whose value changes over time because its just shadowing the original binding instead of using a Value::Ref where a single cells value can actually change instead of shadowing the first binding.

When a block contains a stuck item — say { z; 1 } with z unbound — the block is stuck and the tail 1 is never reached. Walk through how the ? operator in your arm produces that: what it does when a sub-evaluation returns Err(Control::Raise(..)), and which node's span the reported error carries.

5. For {z; 1} z is a Stmt:Expr, so my arm calls self.eval_expr(e, &env)? on it. z is Expr::Var("z", span_of_z), so that inner call returns the UnboundVariable error. The ? immediately returns the UnboundVariable error from the whole block arm and the loop never reaches the 1 item or the tail. The span reported error is z's own span from Expr::Var(_, span) not the blocks span.

# How I implemented it
Walk through your Expr::Block arm: how do you destructure it into its items and optional tail, iterate the items while carrying the growing environment, dispatch on Stmt::Let versus Stmt::Expr, and produce the block's value (including Value::Unit when there is no tail)? Naming the provided tools you used (self.eval_expr, env.lookup, env.extend, RuntimeError, .into()/?) is encouraged. Note that the Option<Ty> annotation on a let is ignored at this milestone.

1. Expr::Block(stms, tail, _span) is pattern matching its checking if this Expr a Block variant and if it is, it unpacks the tree fields into three new local variables. let mut env = env.clone(); shadows the parameter so theres now a local variable called env thats seprate from the env: &env parameter, and its declared mut because it gets reassinged inisde the loop. In for stmt in stmts, stmts is a &Vec<Stmt> so this loop does one &Stmt at a time, in order. The innner match stmt is an enum with two variants(let and expr), so the match dispathes on which kind of statement its looking at. let v = self.eval_expr(rhs, &env)? calls eval_expr recursively on the rhs evaluating it in the current environment and returns a Result<Value, Control>. env.extend(name.clone(), v); is a reasignment not a mutation of the old environments contents. Sstmt::Expr(e) => {self.eval_expr(e, &env)?} is the same idea as when ? is used above but the resulting value is thrown away. After the loop, tail is &Option<Box<Expr>>, and Some(t) means there is a trailing expression and it evaluates the tail in the final env, while None means there is no trailing expression and returns OK(Value::Unit). my Stmt::Let(name, _ty, rhs, _let_span) ignores the Option<Ty> using _ty.

Point out anything that was tricky, a bug you fixed, or a design choice you made — and if you used an assistant, say what you had it do and how you checked its work.

2. A tricky bit for me was I initially tried to lookup and overwrite rather than extend and rebind. I also forgot the ? operator on the Stmt::Expr case and had the example {z; 1} case get stuck silently. I used an assistant to help me find the bugs and point me in the right direction of solving it, and I checkied its work by tracing the stuck example by hand.

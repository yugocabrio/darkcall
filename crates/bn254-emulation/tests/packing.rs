//! Mutate committed source advice while AssignedCell cached Values remain zero.
//! Without copy constraints pack_bytes could silently hash its cached old bytes.
use darkcall_bn254_emulation::Config;
use ff::Field;
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        Advice, Any, Assigned, Assignment, Circuit, Column, ConstraintSystem, Error, Fixed,
        FloorPlanner, Instance, Selector,
    },
};
use pasta_curves::pallas;
struct Mutator<'a, F: Field, CS: Assignment<F>> {
    inner: &'a mut CS,
    changed: bool,
    source_region: bool,
    done: bool,
    marker: std::marker::PhantomData<F>,
}
impl<F: Field, CS: Assignment<F>> Assignment<F> for Mutator<'_, F, CS> {
    fn enter_region<NR: Into<String>, N: FnOnce() -> NR>(&mut self, name_fn: N) {
        let name = name_fn().into();
        self.source_region = name == "external byte" && !self.done;
        self.inner.enter_region(|| name);
    }
    fn exit_region(&mut self) {
        if self.source_region {
            self.done = true;
        }
        self.source_region = false;
        self.inner.exit_region();
    }
    fn enable_selector<A: FnOnce() -> AR, AR: Into<String>>(
        &mut self,
        annotation: A,
        selector: &Selector,
        row: usize,
    ) -> Result<(), Error> {
        self.inner.enable_selector(annotation, selector, row)
    }
    fn query_instance(&self, column: Column<Instance>, row: usize) -> Result<Value<F>, Error> {
        self.inner.query_instance(column, row)
    }
    fn assign_advice<
        V: FnOnce() -> Value<VR>,
        VR: Into<Assigned<F>>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    >(
        &mut self,
        annotation: A,
        column: Column<Advice>,
        row: usize,
        to: V,
    ) -> Result<(), Error> {
        let name = annotation().into();
        let change = self.source_region && name == "external byte" && !self.changed;
        if change {
            self.changed = true;
            self.inner
                .assign_advice(|| name, column, row, || to().map(|v| v.into() + F::ONE))
        } else {
            self.inner.assign_advice(|| name, column, row, to)
        }
    }
    fn assign_fixed<
        V: FnOnce() -> Value<VR>,
        VR: Into<Assigned<F>>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    >(
        &mut self,
        annotation: A,
        column: Column<Fixed>,
        row: usize,
        to: V,
    ) -> Result<(), Error> {
        self.inner.assign_fixed(annotation, column, row, to)
    }
    fn copy(
        &mut self,
        left: Column<Any>,
        left_row: usize,
        right: Column<Any>,
        right_row: usize,
    ) -> Result<(), Error> {
        self.inner.copy(left, left_row, right, right_row)
    }
    fn fill_from_row(
        &mut self,
        column: Column<Fixed>,
        row: usize,
        to: Value<Assigned<F>>,
    ) -> Result<(), Error> {
        self.inner.fill_from_row(column, row, to)
    }
    fn push_namespace<NR: Into<String>, N: FnOnce() -> NR>(&mut self, name_fn: N) {
        self.inner.push_namespace(name_fn);
    }
    fn pop_namespace(&mut self, name: Option<String>) {
        self.inner.pop_namespace(name);
    }
}
#[derive(Clone)]
struct TamperPlanner;
impl FloorPlanner for TamperPlanner {
    fn synthesize<F: Field, CS: Assignment<F>, C: Circuit<F>>(
        cs: &mut CS,
        circuit: &C,
        config: C::Config,
        constants: Vec<Column<Fixed>>,
    ) -> Result<(), Error> {
        let mut wrapper = Mutator {
            inner: cs,
            changed: false,
            source_region: false,
            done: false,
            marker: std::marker::PhantomData,
        };
        SimpleFloorPlanner::synthesize(&mut wrapper, circuit, config, constants)?;
        assert!(
            wrapper.changed,
            "test must actually change a committed source byte"
        );
        Ok(())
    }
}

struct TestCircuit<P>(std::marker::PhantomData<P>);
impl<P> Clone for TestCircuit<P> {
    fn clone(&self) -> Self {
        Self(std::marker::PhantomData)
    }
}
impl<P: FloorPlanner> Circuit<pallas::Base> for TestCircuit<P> {
    type Config = (Config, Column<Advice>, Column<Instance>);
    type FloorPlanner = P;
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
        let config = Config::configure(meta);
        let source = meta.advice_column();
        meta.enable_equality(source);
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        (config, source, instance)
    }
    fn synthesize(
        &self,
        (config, source, instance): Self::Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        config.load(&mut layouter)?;
        let mut bytes = Vec::new();
        for _ in 0..30 {
            bytes.push(layouter.assign_region(
                || "external byte",
                |mut region| {
                    region.assign_advice(
                        || "external byte",
                        source,
                        0,
                        || Value::known(pallas::Base::ZERO),
                    )
                },
            )?);
        }
        let element = config.pack_bytes(&mut layouter, &bytes)?;
        for (i, cell) in element.limbs.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), instance, i)?;
        }
        Ok(())
    }
}
#[test]
fn cached_value_cannot_replace_committed_source_byte() {
    let good = TestCircuit::<SimpleFloorPlanner>(std::marker::PhantomData);
    MockProver::run(10, &good, vec![vec![pallas::Base::ZERO; 4]])
        .unwrap()
        .assert_satisfied();
    let bad = TestCircuit::<TamperPlanner>(std::marker::PhantomData);
    let violations = MockProver::run(10, &bad, vec![vec![pallas::Base::ZERO; 4]])
        .unwrap()
        .verify()
        .unwrap_err();
    assert!(
        violations
            .iter()
            .any(|v| matches!(v, halo2_proofs::dev::VerifyFailure::Permutation { .. }))
    );
}

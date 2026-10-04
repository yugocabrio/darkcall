//! Mutate the committed advice table while keeping AssignedCell cached values
//! unchanged. This detects a missing copy constraint that normal hash vectors
//! cannot detect: both the external word and its bytes are changed consistently,
//! while the SHA internal witness still hashes the original cached zero word.
use darkcall_sha_link::{Cell, Config};
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
use sha2::{Digest, Sha256};

struct Mutator<'a, F: Field, CS: Assignment<F>> {
    inner: &'a mut CS,
    word_changed: bool,
    byte_changed: bool,
    source_region: bool,
    done: bool,
    marker: std::marker::PhantomData<F>,
}
impl<F: Field, CS: Assignment<F>> Assignment<F> for Mutator<'_, F, CS> {
    fn enter_region<NR: Into<String>, N: FnOnce() -> NR>(&mut self, name_fn: N) {
        let name = name_fn().into();
        self.source_region = name == "source word" && !self.done;
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
        let change = self.source_region
            && ((name == "source word" && !self.word_changed)
                || (name == "word byte" && !self.byte_changed));
        if change {
            if name == "source word" {
                self.word_changed = true;
            } else {
                self.byte_changed = true;
            }
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
            word_changed: false,
            byte_changed: false,
            source_region: false,
            done: false,
            marker: std::marker::PhantomData,
        };
        SimpleFloorPlanner::synthesize(&mut wrapper, circuit, config, constants)?;
        assert!(
            wrapper.word_changed && wrapper.byte_changed,
            "test must actually mutate a coherent input word"
        );
        Ok(())
    }
}
struct TestCircuit<P> {
    planner: std::marker::PhantomData<P>,
}
impl<P> Clone for TestCircuit<P> {
    fn clone(&self) -> Self {
        Self {
            planner: std::marker::PhantomData,
        }
    }
}
impl<P: FloorPlanner> Circuit<pallas::Base> for TestCircuit<P> {
    type Config = (Config, Column<Instance>);
    type FloorPlanner = P;
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
        let chip = Config::configure(meta);
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        (chip, instance)
    }
    fn synthesize(
        &self,
        (chip, instance): Self::Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        chip.load(&mut layouter)?;
        let mut words = Vec::new();
        for _ in 0..56 {
            words.push(chip.word(&mut layouter, Value::known(0))?);
        }
        let words: [Cell; 56] = words.try_into().unwrap();
        let digest = chip.hash_224(&mut layouter, &words)?;
        for (i, cell) in digest.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), instance, i)?;
        }
        Ok(())
    }
}
#[test]
fn coherent_external_input_mutation_is_rejected_by_copy_constraints() {
    let digest = Sha256::digest([0u8; 224]);
    let public: Vec<_> = digest
        .chunks_exact(4)
        .map(|x| pallas::Base::from(u32::from_be_bytes(x.try_into().unwrap()) as u64))
        .collect();
    let good = TestCircuit::<SimpleFloorPlanner> {
        planner: std::marker::PhantomData,
    };
    MockProver::run(17, &good, vec![public.clone()])
        .unwrap()
        .assert_satisfied();
    let bad = TestCircuit::<TamperPlanner> {
        planner: std::marker::PhantomData,
    };
    let violations = MockProver::run(17, &bad, vec![public])
        .unwrap()
        .verify()
        .unwrap_err();
    assert!(
        violations.iter().any(|failure| matches!(
            failure,
            halo2_proofs::dev::VerifyFailure::Permutation { .. }
        )),
        "coherent source change must fail a copy constraint: {violations:?}"
    );
}

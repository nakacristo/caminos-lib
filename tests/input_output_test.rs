mod common;
use caminos_lib::*;
use ::rand::{rngs::StdRng};
use rand::SeedableRng;
use caminos_lib::config_parser::ConfigurationValue;
use common::*;
use std::rc::Rc;
use std::cell::RefCell;
use caminos_lib::routing::RoutingInfo;

#[test]
fn input_output_switch_local_traffic()
{

    let mut rng=StdRng::seed_from_u64(10u64);
    let plugs = Plugs::default();

    let n_servers = 2.0;
    let messages_per_server = 12.0;
    let message_size = 16.0;
    let cycles = 200.0;

    let estimated_injected_load = message_size * messages_per_server / cycles;

    let topology = create_hamming_topology(vec![ConfigurationValue::Number(1f64)], 2f64, &mut rng);
    let pattern = create_shift_pattern(vec![ConfigurationValue::Number(2f64),ConfigurationValue::Number(1f64)], vec![ConfigurationValue::Number(1f64), ConfigurationValue::Number(0f64)]);
    let traffic = create_burst_traffic(pattern, n_servers, messages_per_server, message_size);
    let vcp = create_vcp();
    let router = create_input_output_router(1.0, vcp, 0.0, ConfigurationValue::Object("Random".to_string(), vec![("seed".to_string(), ConfigurationValue::Number(1f64))]), 64.0, ConfigurationValue::False, 16.0, ConfigurationValue::True, ConfigurationValue::False, 32.0, ConfigurationValue::False);
    let routing = create_shortest_routing();
    let link_classes = create_link_classes();

    let simulation_cv = ConfigurationValue::Object("Configuration".to_string(), vec![
        ("random_seed".to_string(), ConfigurationValue::Number(1.0)),
        ("warmup".to_string(), ConfigurationValue::Number(0.0)),
        ("measured".to_string(), ConfigurationValue::Number(cycles)),
        ("topology".to_string(), topology),
        ("traffic".to_string(), traffic),
        ("router".to_string(), router),
        ("maximum_packet_size".to_string(), ConfigurationValue::Number(16.0)),
        ("general_frequency_divisor".to_string(), ConfigurationValue::Number(1.0)),
        ("routing".to_string(), routing),
        ("link_classes".to_string(), link_classes),
    ]);
    // println!("{:#?}", simulation_cv);
    let mut simulation = Simulation::new(&simulation_cv, &plugs);
    simulation.run();
    let results = simulation.get_simulation_results();
    println!("{:#?}", results);

    match_object_panic!( &results, "Result", value,
        "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );
}

#[test]
fn input_output_two_servers_two_routers()
{

    let mut rng=StdRng::seed_from_u64(10u64);
    let plugs = Plugs::default();


    let n_servers = 2.0;
    let messages_per_server = 12.0;
    let message_size = 16.0;
    let cycles = 200.0;

    let estimated_injected_load = message_size * messages_per_server / cycles;

    let topology = create_hamming_topology(vec![ConfigurationValue::Number(2f64)], 1f64, &mut rng);
    let pattern = create_shift_pattern(vec![ConfigurationValue::Number(1f64),ConfigurationValue::Number(2f64)], vec![ConfigurationValue::Number(0f64), ConfigurationValue::Number(1f64)]);
    let traffic = create_burst_traffic(pattern, n_servers, messages_per_server, message_size);
    let vcp = create_vcp();
    let router = create_input_output_router(1.0, vcp, 0.0, ConfigurationValue::Object("Random".to_string(), vec![("seed".to_string(), ConfigurationValue::Number(1f64))]), 64.0, ConfigurationValue::False, 16.0, ConfigurationValue::True, ConfigurationValue::False, 32.0, ConfigurationValue::False);
    let routing = create_shortest_routing();
    let link_classes = create_link_classes();

    let simulation_cv = ConfigurationValue::Object("Configuration".to_string(), vec![
        ("random_seed".to_string(), ConfigurationValue::Number(1.0)),
        ("warmup".to_string(), ConfigurationValue::Number(0.0)),
        ("measured".to_string(), ConfigurationValue::Number(cycles)),
        ("topology".to_string(), topology),
        ("traffic".to_string(), traffic),
        ("router".to_string(), router),
        ("maximum_packet_size".to_string(), ConfigurationValue::Number(16.0)),
        ("general_frequency_divisor".to_string(), ConfigurationValue::Number(1.0)),
        ("routing".to_string(), routing),
        ("link_classes".to_string(), link_classes),
    ]);
    // println!("{:#?}", simulation_cv);
    let mut simulation = Simulation::new(&simulation_cv, &plugs);
    simulation.run();
    let results = simulation.get_simulation_results();
    println!("{:#?}", results);

    match_object_panic!( &results, "Result", value,
        "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );

}
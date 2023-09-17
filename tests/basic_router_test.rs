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
fn output_buffer_test()
{
    //check that the output buffers are working correctly
    todo!()
}


#[test]
fn basic_switch_local_traffic()
{

    let mut rng=StdRng::seed_from_u64(10u64);
    let plugs = Plugs::default();

    let n_servers = 2.0;
    let messages_per_server = 1.0;
    let message_size = 16.0;
    let cycles = 18.0; //This is when it should end (?) Nic-switch (1 cycle) + switch-Nic (1 cycle)

    let estimated_injected_load =  message_size * messages_per_server / cycles; // Aprox... Maybe not the best value now but it is a start
    let packet_hops = 0.0;

    let topology = create_hamming_topology(vec![ConfigurationValue::Number(1f64)], 2f64, &mut rng);
    let pattern = create_shift_pattern(vec![ConfigurationValue::Number(2f64),ConfigurationValue::Number(1f64)], vec![ConfigurationValue::Number(1f64), ConfigurationValue::Number(0f64)]);
    let traffic = create_burst_traffic(pattern, n_servers, messages_per_server, message_size);
    let vcp = create_vcp();
    let router = create_basic_router(1.0, vcp, 0.0, 64.0, ConfigurationValue::False, 16.0, ConfigurationValue::True, ConfigurationValue::False, 32.0, ConfigurationValue::False,ConfigurationValue::False);
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
        "accepted_load" => assert_eq!(value.as_f64().expect("Accepted load load data"), estimated_injected_load), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "average_packet_hops" => assert_eq!(value.as_f64().expect("Packet hops data"), packet_hops), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );
}


#[test]
fn basic_two_servers_two_routers()
{

    let mut rng=StdRng::seed_from_u64(10u64);
    let plugs = Plugs::default();

    let n_servers = 2.0;
    let messages_per_server = 1.0;
    let message_size = 16.0;
    let cycles = 19.0; //This is when it should end (?) Nic-switch + router-router + switch-Nic

    let estimated_injected_load =  message_size * messages_per_server / cycles; // Aprox... Maybe not the best value now but it is a start
    let packet_hops = 1.0;

    let topology = create_hamming_topology(vec![ConfigurationValue::Number(2f64)], 1f64, &mut rng);
    let pattern = create_shift_pattern(vec![ConfigurationValue::Number(1f64),ConfigurationValue::Number(2f64)], vec![ConfigurationValue::Number(0f64), ConfigurationValue::Number(1f64)]);
    let traffic = create_burst_traffic(pattern, n_servers, messages_per_server, message_size);
    let vcp = create_vcp();
    let router = create_basic_router(1.0, vcp, 0.0, 64.0, ConfigurationValue::False, 16.0, ConfigurationValue::True, ConfigurationValue::False, 32.0, ConfigurationValue::False,ConfigurationValue::False);
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
        "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load, "Injection load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "accepted_load" => assert_eq!(value.as_f64().expect("Accepted load load data"), estimated_injected_load), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "average_packet_hops" => assert_eq!(value.as_f64().expect("Packet hops data"), packet_hops), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );
}





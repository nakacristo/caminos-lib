/*!
    Tests for the Input_output router
*/

mod common;
use caminos_lib::*;
use caminos_lib::config_parser::ConfigurationValue;
use common::*;

/// Test local traffic inside a router. There are two servers and each server sends one message of 16 phits to each other.
/// We check that the values obtained in the simulation `[cycle (latency), accepted_load, injected_load, average_packet_hops]` are the expected ones.
#[test]
fn input_output_switch_local_traffic()
{
    // Hamming
    let network_sides = vec![1];
    let servers_per_router = 2;
    let hamming_builder = HammingBuilder{
        sides: network_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64) ).collect(), //vec![ConfigurationValue::Number(1.0)],
        servers_per_router,
    };

    //Pattern
    let total_sides = vec![2, 1]; //sides of the Cartesian pattern
    let cartesian_shift = vec![1, 0]; //shift of the Cartesian pattern
    let shift_pattern_builder = ShiftPatternBuilder{
        sides: total_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(), //vec![ConfigurationValue::Number(2.0),ConfigurationValue::Number(1.0)],
        shift: cartesian_shift.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(),//vec![ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0)],
    };
    let pattern = create_shift_pattern(shift_pattern_builder);

    // Burst traffic
    let servers = 2;
    let messages_per_server = 1;
    let message_size = 16;
    let burst_traffic_builder = BurstTrafficBuilder{
            pattern,
            servers,
            messages_per_server,
            message_size,

    };

    //Virtual Channel Policies
    let vcp_args = VirtualChannelPoliciesBuilder{
        policies: vec![
            ConfigurationValue::Object("LowestLabel".to_string(), vec![]),
            ConfigurationValue::Object("EnforceFlowControl".to_string(), vec![]),
            ConfigurationValue::Object("Random".to_string(), vec![])
        ]
    };
    let vcp = create_vcp(vcp_args);


    //Router Input output
    let crossbar_delay = 1;
    let crossbar_frequency_divisor = 1;
    let router_args = InputOutputRouterBuilder{
        virtual_channels: 1,
        vcp,
        crossbar_delay,
        crossbar_frequency_divisor,
        allocator: ConfigurationValue::Object("Random".to_string(), vec![("seed".to_string(), ConfigurationValue::Number(1f64))]),
        buffer_size: 64,
        bubble: ConfigurationValue::False,
        flit_size: message_size, //vct
        allow_request_busy_port: ConfigurationValue::True,
        intransit_priority: ConfigurationValue::False,
        output_buffer_size: 32,
        neglect_busy_outport: ConfigurationValue::False,
    };


    let cycles = crossbar_delay + messages_per_server * message_size + 2; //+2 is because of the switch-Nic and Nic-switch links which take one cycle each
    let maximum_packet_size=16;

    let topology = create_hamming_topology(hamming_builder);
    let traffic = create_burst_traffic(burst_traffic_builder);
    let router = create_input_output_router(router_args);
    let routing = create_shortest_routing();
    let link_classes = create_link_classes();

    let simulation_builder = SimulationBuilder{
        random_seed: 1,
        warmup: 0,
        measured: cycles,
        topology,
        traffic,
        router,
        maximum_packet_size,
        general_frequency_divisor: 1,
        routing,
        link_classes
    };

    let plugs = Plugs::default();
    let simulation_cv = create_simulation(simulation_builder);

    // println!("{:#?}", simulation_cv);
    let mut simulation = Simulation::new(&simulation_cv, &plugs);
    simulation.run();
    let results = simulation.get_simulation_results();
    println!("{:#?}", results);

    let estimated_injected_load =  (message_size * messages_per_server) as f64 / (cycles as f64); // Aprox... Maybe not the best value now but it is a start
    let packet_hops = 0.0;

    match_object_panic!( &results, "Result", value,
        "cycle" => assert_eq!(value.as_f64().expect("Cycle data"), cycles as f64, "Cycle"),
        "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load, "Injected load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "accepted_load" => assert_eq!(value.as_f64().expect("Accepted load load data"), estimated_injected_load, "Accepted load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "average_packet_hops" => assert_eq!(value.as_f64().expect("Packet hops data"), packet_hops, "Total hops"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );
}


/// Test traffic between routers. There are two servers and each server is connected to a different router. The send one message of 16 phits to each other.
/// We check that the values obtained in the simulation `[cycle (latency), accepted_load, injected_load, average_packet_hops]` are the expected ones.
#[test]
fn input_output_two_servers_two_routers()
{

    // Hamming
    let network_sides = vec![2];
    let servers_per_router = 1;
    let hamming_builder = HammingBuilder{
        sides: network_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64) ).collect(), //vec![ConfigurationValue::Number(1.0)],
        servers_per_router,
    };

    //Pattern
    let total_sides = vec![1, 2]; //sides of the Cartesian pattern
    let cartesian_shift = vec![0, 1]; //shift of the Cartesian pattern
    let shift_pattern_builder = ShiftPatternBuilder{
        sides: total_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(), //vec![ConfigurationValue::Number(2.0),ConfigurationValue::Number(1.0)],
        shift: cartesian_shift.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(),//vec![ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0)],
    };
    let pattern = create_shift_pattern(shift_pattern_builder);

    // Burst traffic
    let servers = 2;
    let messages_per_server = 1;
    let message_size = 16;
    let burst_traffic_builder = BurstTrafficBuilder{
        pattern,
        servers,
        messages_per_server,
        message_size,

    };

    //Virtual Channel Policies
    let vcp_args = VirtualChannelPoliciesBuilder{
        policies: vec![
            ConfigurationValue::Object("LowestLabel".to_string(), vec![]),
            ConfigurationValue::Object("EnforceFlowControl".to_string(), vec![]),
            ConfigurationValue::Object("Random".to_string(), vec![])
        ]
    };
    let vcp = create_vcp(vcp_args);


    //Router Input output
    let crossbar_delay = 1;
    let crossbar_frequency_divisor = 1;
    let router_args = InputOutputRouterBuilder{
        virtual_channels: 1,
        vcp,
        crossbar_delay,
        crossbar_frequency_divisor,
        allocator: ConfigurationValue::Object("Random".to_string(), vec![("seed".to_string(), ConfigurationValue::Number(1f64))]),
        buffer_size: 64,
        bubble: ConfigurationValue::False,
        flit_size: message_size, //vct
        allow_request_busy_port: ConfigurationValue::True,
        intransit_priority: ConfigurationValue::False,
        output_buffer_size: 32,
        neglect_busy_outport: ConfigurationValue::False,
    };


    let cycles = 2 * crossbar_delay + messages_per_server * message_size + 3; //+3 is because of the switch-Nic + switch-switch + Nic-switch links which take one cycle each
    let maximum_packet_size=16;

    let topology = create_hamming_topology(hamming_builder);
    let traffic = create_burst_traffic(burst_traffic_builder);
    let router = create_input_output_router(router_args);
    let routing = create_shortest_routing();
    let link_classes = create_link_classes();

    let simulation_builder = SimulationBuilder{
        random_seed: 1,
        warmup: 0,
        measured: cycles,
        topology,
        traffic,
        router,
        maximum_packet_size,
        general_frequency_divisor: 1,
        routing,
        link_classes
    };

    let plugs = Plugs::default();
    let simulation_cv = create_simulation(simulation_builder);

    // println!("{:#?}", simulation_cv);
    let mut simulation = Simulation::new(&simulation_cv, &plugs);
    simulation.run();
    let results = simulation.get_simulation_results();
    println!("{:#?}", results);

    let estimated_injected_load =  (message_size * messages_per_server) as f64 / (cycles as f64); // Aprox... Maybe not the best value now but it is a start
    let packet_hops = 1.0;

    match_object_panic!( &results, "Result", value,
        "cycle" => assert_eq!(value.as_f64().expect("Cycle data"), cycles as f64, "Cycle"),
        "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load, "Injected load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "accepted_load" => assert_eq!(value.as_f64().expect("Accepted load load data"), estimated_injected_load, "Accepted load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "average_packet_hops" => assert_eq!(value.as_f64().expect("Packet hops data"), packet_hops, "Total hops"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );

}



/// Test traffic between routers. There are two servers and each server is connected to a different router. The send one message of 16 phits to each other. The routers have a frequency divisor (speedup) of x2.
/// We check that the values obtained in the simulation `[cycle (latency), accepted_load, injected_load, average_packet_hops]` are the expected ones.
#[test]
fn input_output_internal_speedup()
{

    // Hamming
    let network_sides = vec![2];
    let servers_per_router = 1;
    let hamming_builder = HammingBuilder{
        sides: network_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64) ).collect(), //vec![ConfigurationValue::Number(1.0)],
        servers_per_router,
    };

    //Pattern
    let total_sides = vec![1, 2]; //sides of the Cartesian pattern
    let cartesian_shift = vec![0, 1]; //shift of the Cartesian pattern
    let shift_pattern_builder = ShiftPatternBuilder{
        sides: total_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(), //vec![ConfigurationValue::Number(2.0),ConfigurationValue::Number(1.0)],
        shift: cartesian_shift.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(),//vec![ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0)],
    };
    let pattern = create_shift_pattern(shift_pattern_builder);

    // Burst traffic
    let servers = 2;
    let messages_per_server = 1;
    let message_size = 16;
    let burst_traffic_builder = BurstTrafficBuilder{
        pattern,
        servers,
        messages_per_server,
        message_size,

    };

    //Virtual Channel Policies
    let vcp_args = VirtualChannelPoliciesBuilder{
        policies: vec![
            ConfigurationValue::Object("LowestLabel".to_string(), vec![]),
            ConfigurationValue::Object("EnforceFlowControl".to_string(), vec![]),
            ConfigurationValue::Object("Random".to_string(), vec![])
        ]
    };
    let vcp = create_vcp(vcp_args);


    //Router Input output
    let crossbar_delay = 1;
    let crossbar_frequency_divisor = 1;
    let router_args = InputOutputRouterBuilder{
        virtual_channels: 1,
        vcp,
        crossbar_delay,
        crossbar_frequency_divisor,
        allocator: ConfigurationValue::Object("Random".to_string(), vec![("seed".to_string(), ConfigurationValue::Number(1f64))]),
        buffer_size: 64,
        bubble: ConfigurationValue::False,
        flit_size: message_size, //vct
        allow_request_busy_port: ConfigurationValue::True,
        intransit_priority: ConfigurationValue::False,
        output_buffer_size: 32,
        neglect_busy_outport: ConfigurationValue::False,
    };

    let general_frequency_divisor = 2;
    let cycles = general_frequency_divisor * (messages_per_server * (message_size-1) ) + 6; //+3 is because of the switch-Nic + switch-switch + Nic-switch links which take one cycle each
    let maximum_packet_size=16;

    let topology = create_hamming_topology(hamming_builder);
    let traffic = create_burst_traffic(burst_traffic_builder);
    let router = create_input_output_router(router_args);
    let routing = create_shortest_routing();
    let link_classes = create_link_classes();

    let simulation_builder = SimulationBuilder{
        random_seed: 1,
        warmup: 0,
        measured: cycles,
        topology,
        traffic,
        router,
        maximum_packet_size,
        general_frequency_divisor,
        routing,
        link_classes
    };

    let plugs = Plugs::default();
    let simulation_cv = create_simulation(simulation_builder);

    // println!("{:#?}", simulation_cv);
    let mut simulation = Simulation::new(&simulation_cv, &plugs);
    simulation.run();
    let results = simulation.get_simulation_results();
    println!("{:#?}", results);

    let estimated_injected_load =  (message_size * messages_per_server) as f64 / (cycles as f64); // Aprox... Maybe not the best value now but it is a start
    let packet_hops = 1.0;

    match_object_panic!( &results, "Result", value,
        "cycle" => assert_eq!(value.as_f64().expect("Cycle data"), cycles as f64, "Cycle"),
        "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load, "Injected load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "accepted_load" => assert_eq!(value.as_f64().expect("Accepted load load data"), estimated_injected_load, "Accepted load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "average_packet_hops" => assert_eq!(value.as_f64().expect("Packet hops data"), packet_hops, "Total hops"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );

}

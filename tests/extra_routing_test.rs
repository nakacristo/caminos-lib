/*!
Tests for the Sum routing
 */

mod common;

use itertools::assert_equal;
use caminos_lib::*;
use caminos_lib::config_parser::ConfigurationValue;
use common::*;

/// Test local traffic inside a router. There are two servers and each server sends one message of 16 phits to each other.
/// We check that the values obtained in the simulation `[cycle (latency), accepted_load, injected_load, average_packet_hops]` are the expected ones.
#[test]
fn sum_routing_test_1()
{
    // Hamming
    let network_sides = vec![2,2];
    let servers_per_router = 1;
    let hamming_builder = HammingBuilder{
        sides: network_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64) ).collect(), //vec![ConfigurationValue::Number(1.0)],
        servers_per_router,
    };

    //Pattern
    let total_sides = vec![1, 2, 2]; //sides of the Cartesian pattern
    let cartesian_shift = vec![0, 1, 1]; //shift of the Cartesian pattern
    let shift_pattern_builder = ShiftPatternBuilder{
        sides: total_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(), //vec![ConfigurationValue::Number(2.0),ConfigurationValue::Number(1.0)],
        shift: cartesian_shift.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(),//vec![ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0)],
    };
    let pattern = create_shift_pattern(shift_pattern_builder);

    // Burst traffic
    let servers = 4;
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
            ConfigurationValue::Object("MapLabel".to_string(), vec![
                ("label_to_policy".to_string(), ConfigurationValue::Array(
                    vec![ConfigurationValue::Object("LabelSaturate".to_string(), vec![
                        ("value".to_string(), ConfigurationValue::Number(5.0)), ("bottom".to_string(), ConfigurationValue::True)]), //0
                         ConfigurationValue::Object("Identity".to_string(), vec![]), // 1
                         ConfigurationValue::Object("LabelSaturate".to_string(), vec![
                        ("value".to_string(), ConfigurationValue::Number(5.0)), ("bottom".to_string(), ConfigurationValue::True)]), //2
                    ]))
            ]),
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
        virtual_channels: 2,
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


    let cycles = 100;//crossbar_delay + messages_per_server * message_size + 2; //+2 is because of the switch-Nic and Nic-switch links which take one cycle each
    let maximum_packet_size=16;

    let topology = create_hamming_topology(hamming_builder);
    let traffic = create_burst_traffic(burst_traffic_builder);
    let router = create_input_output_router(router_args);
    let routing = ConfigurationValue::Object("Sum".to_string(), vec![
        ("policy".to_string(), ConfigurationValue::Object("TryBoth".to_string(), vec![])),
        ("first_routing".to_string(), create_dor_routing(vec![1,0])),
        ("second_routing".to_string(), create_dor_routing(vec![1,0])),
        ("first_allowed_virtual_channels".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(0.0)] )),
        ("second_allowed_virtual_channels".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(1.0)] )),
        ("same_port_extra_label".to_string(), ConfigurationValue::Number(1.0)),
        ("first_extra_label".to_string(), ConfigurationValue::Number(1.0)),
    ]);//create_shortest_routing();
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

    let packet_hops = 2.0;

    match_object_panic!( &results, "Result", value,
        // "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load, "Injected load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        // "accepted_load" => assert_eq!(value.as_f64().expect("Accepted load load data"), estimated_injected_load, "Accepted load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "average_packet_hops" => assert_eq!(value.as_f64().expect("Packet hops data"), packet_hops, "Total hops"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "virtual_channel_usage" => assert_eq!(value.as_array().expect("Virtual channel usage data").iter().map(|a| a.as_f64().expect("Virtual channel usage data")).collect::<Vec<f64>>()[0],0.0, "Virtual channel usage"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );
}


/// Test local traffic inside a router. There are two servers and each server sends one message of 16 phits to each other.
/// We check that the values obtained in the simulation `[cycle (latency), accepted_load, injected_load, average_packet_hops]` are the expected ones.
#[test]
fn sum_routing_test_2()
{
    // Hamming
    let network_sides = vec![2,2];
    let servers_per_router = 1;
    let hamming_builder = HammingBuilder{
        sides: network_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64) ).collect(), //vec![ConfigurationValue::Number(1.0)],
        servers_per_router,
    };

    //Pattern
    let total_sides = vec![1, 2, 2]; //sides of the Cartesian pattern
    let cartesian_shift = vec![0, 1, 1]; //shift of the Cartesian pattern
    let shift_pattern_builder = ShiftPatternBuilder{
        sides: total_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(), //vec![ConfigurationValue::Number(2.0),ConfigurationValue::Number(1.0)],
        shift: cartesian_shift.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(),//vec![ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0)],
    };
    let pattern = create_shift_pattern(shift_pattern_builder);

    // Burst traffic
    let servers = 4;
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
            ConfigurationValue::Object("MapLabel".to_string(), vec![
                ("label_to_policy".to_string(), ConfigurationValue::Array(
                    vec![ConfigurationValue::Object("LabelSaturate".to_string(), vec![
                        ("value".to_string(), ConfigurationValue::Number(5.0)), ("bottom".to_string(), ConfigurationValue::True)]), //0
                         ConfigurationValue::Object("Identity".to_string(), vec![]), // 1
                         ConfigurationValue::Object("LabelSaturate".to_string(), vec![
                             ("value".to_string(), ConfigurationValue::Number(5.0)), ("bottom".to_string(), ConfigurationValue::True)]), //2
                    ]))
            ]),
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
        virtual_channels: 2,
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


    let cycles = 100;//crossbar_delay + messages_per_server * message_size + 2; //+2 is because of the switch-Nic and Nic-switch links which take one cycle each
    let maximum_packet_size=16;

    let topology = create_hamming_topology(hamming_builder);
    let traffic = create_burst_traffic(burst_traffic_builder);
    let router = create_input_output_router(router_args);
    let routing = ConfigurationValue::Object("Sum".to_string(), vec![
        ("policy".to_string(), ConfigurationValue::Object("TryBoth".to_string(), vec![])),
        ("first_routing".to_string(), create_dor_routing(vec![0,1])),
        ("second_routing".to_string(), create_dor_routing(vec![1,0])),
        ("first_allowed_virtual_channels".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(0.0)] )),
        ("second_allowed_virtual_channels".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(1.0)] )),
        ("same_port_extra_label".to_string(), ConfigurationValue::Number(1.0)),
        ("first_extra_label".to_string(), ConfigurationValue::Number(1.0)),
        ("second_extra_label".to_string(), ConfigurationValue::Number(1.0)),
    ]);//create_shortest_routing();
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

    let packet_hops = 2.0;

    match_object_panic!( &results, "Result", value,
        // "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load, "Injected load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        // "accepted_load" => assert_eq!(value.as_f64().expect("Accepted load load data"), estimated_injected_load, "Accepted load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "average_packet_hops" => assert_eq!(value.as_f64().expect("Packet hops data"), packet_hops, "Total hops"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "virtual_channel_usage" => assert_eq!(value.clone().as_array().expect("Virtual channel usage data").iter().map(|a| a.as_f64().expect("Virtual channel usage data")).collect::<Vec<f64>>()[0], value.clone().as_array().expect("Virtual channel usage data").iter().map(|a| a.as_f64().expect("Virtual channel usage data")).collect::<Vec<f64>>()[1], "Virtual channel usage"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );
}


/// Test local traffic inside a router. There are two servers and each server sends one message of 16 phits to each other.
/// We check that the values obtained in the simulation `[cycle (latency), accepted_load, injected_load, average_packet_hops]` are the expected ones.
#[test]
fn sum_routing_test_3()
{
    // Hamming
    let network_sides = vec![2,2];
    let servers_per_router = 1;
    let hamming_builder = HammingBuilder{
        sides: network_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64) ).collect(), //vec![ConfigurationValue::Number(1.0)],
        servers_per_router,
    };

    //Pattern
    let total_sides = vec![1, 2, 2]; //sides of the Cartesian pattern
    let cartesian_shift = vec![0, 1, 1]; //shift of the Cartesian pattern
    let shift_pattern_builder = ShiftPatternBuilder{
        sides: total_sides.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(), //vec![ConfigurationValue::Number(2.0),ConfigurationValue::Number(1.0)],
        shift: cartesian_shift.into_iter().map(|a| ConfigurationValue::Number(a as f64)).collect(),//vec![ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0)],
    };
    let pattern = create_shift_pattern(shift_pattern_builder);

    // Burst traffic
    let servers = 4;
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
            ConfigurationValue::Object("MapLabel".to_string(), vec![
                ("label_to_policy".to_string(), ConfigurationValue::Array(
                    vec![ConfigurationValue::Object("LabelSaturate".to_string(), vec![
                        ("value".to_string(), ConfigurationValue::Number(5.0)), ("bottom".to_string(), ConfigurationValue::True)]), //0
                         ConfigurationValue::Object("Identity".to_string(), vec![]), // 1
                         ConfigurationValue::Object("LabelSaturate".to_string(), vec![
                             ("value".to_string(), ConfigurationValue::Number(5.0)), ("bottom".to_string(), ConfigurationValue::True)]), //2
                    ]))
            ]),
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
        virtual_channels: 2,
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


    let cycles = 100;//crossbar_delay + messages_per_server * message_size + 2; //+2 is because of the switch-Nic and Nic-switch links which take one cycle each
    let maximum_packet_size=16;

    let topology = create_hamming_topology(hamming_builder);
    let traffic = create_burst_traffic(burst_traffic_builder);
    let router = create_input_output_router(router_args);
    let routing = ConfigurationValue::Object("Sum".to_string(), vec![
        ("policy".to_string(), ConfigurationValue::Object("TryBoth".to_string(), vec![])),
        ("first_routing".to_string(), create_dor_routing(vec![1,0])),
        ("second_routing".to_string(), create_dor_routing(vec![1,0])),
        ("first_allowed_virtual_channels".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(0.0)] )),
        ("second_allowed_virtual_channels".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(1.0)] )),
        ("same_port_extra_label".to_string(), ConfigurationValue::Number(1.0)),
    ]);//create_shortest_routing();
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

    let packet_hops = 2.0;

    match_object_panic!( &results, "Result", value,
        // "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load, "Injected load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        // "accepted_load" => assert_eq!(value.as_f64().expect("Accepted load load data"), estimated_injected_load, "Accepted load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "average_packet_hops" => assert_eq!(value.as_f64().expect("Packet hops data"), packet_hops, "Total hops"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        "virtual_channel_usage" => assert_eq!(value.clone().as_array().expect("Virtual channel usage data").iter().map(|a| a.as_f64().expect("Virtual channel usage data")).collect::<Vec<f64>>()[0], value.clone().as_array().expect("Virtual channel usage data").iter().map(|a| a.as_f64().expect("Virtual channel usage data")).collect::<Vec<f64>>()[1], "Virtual channel usage"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
        _ => (),
    );
}
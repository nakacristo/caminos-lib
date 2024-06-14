use std::rc::Rc;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use crate::config_parser::ConfigurationValue;
use crate::{match_object_panic, Message, Time};
use crate::topology::Topology;
use crate::traffic::{new_traffic, TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};
use crate::traffic::basic::{build_message_cv, BuildMessageCVArgs};
use crate::traffic::TaskTrafficState::{UnspecifiedWait, WaitingData};



/**
Introduces a barrier when all the tasks has sent a number of messages.
Tasks will generate messages again when all the messages are consumed.
```ignore
MessageBarrier{
	traffic: HomogeneousTraffic{...},
	tasks: 1000,
	messages_per_task_to_wait: 10,
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct MessageBarrier
{
    ///Number of tasks applying this traffic.
    tasks: usize,
    ///Traffic
    traffic: Box<dyn Traffic>,
    ///The number of messages to send per iteration
    messages_per_task_to_wait: usize,
    ///Total sent
    total_sent_per_task: Vec<usize>,
    ///Total sent
    total_sent: usize,
    ///Total consumed
    total_consumed: usize,
    ///Consumed messages in the barrier
    total_consumed_per_task: Vec<usize>,
    ///Messages to consume to go waiting
    expected_messages_to_consume_to_wait: Option<usize>,
}

impl Traffic for MessageBarrier
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        let message = self.traffic.generate_message(origin,cycle,topology,rng);
        if !message.is_err(){
            self.total_sent += 1;
            self.total_sent_per_task[origin] += 1;
        }
        message
    }
    fn probability_per_cycle(&self, task:usize) -> f32 //should i check the task?
    {
        if self.total_sent_per_task[task] <= self.messages_per_task_to_wait {

            self.traffic.probability_per_cycle(task)

        } else {

            0.0
        }
    }

    fn should_generate(self: &mut MessageBarrier, task:usize, cycle:Time, rng: &mut StdRng) -> bool
    {
        self.total_sent_per_task[task] < self.messages_per_task_to_wait && self.traffic.should_generate(task, cycle, rng)
    }

    fn try_consume(&mut self, task:usize, message: Rc<Message>, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        self.total_consumed += 1;
        self.total_consumed_per_task[task] += 1;
        if self.total_sent == self.total_consumed && self.messages_per_task_to_wait * self.tasks == self.total_sent {
            self.total_sent = 0;
            self.total_consumed = 0;
            self.total_sent_per_task = vec![0; self.tasks];
            self.total_consumed_per_task = vec![0; self.tasks];
        }
        self.traffic.try_consume(task, message, cycle, topology, rng)
    }
    fn is_finished(&self) -> bool
    {
        false
    }
    fn task_state(&self, task:usize, cycle:Time) -> Option<TaskTrafficState>
    {
        if self.total_sent_per_task[task] < self.messages_per_task_to_wait {
            self.traffic.task_state(task, cycle)
        } else {
            if let Some(expected_messages_to_consume) = self.expected_messages_to_consume_to_wait {
                return if self.total_consumed_per_task[task] < expected_messages_to_consume {
                    Some(WaitingData)
                } else {
                    Some(UnspecifiedWait)
                }
            }
            Some(UnspecifiedWait)
        }
    }

    fn number_tasks(&self) -> usize {
        self.tasks
    }
}

impl MessageBarrier
{
    pub fn new(mut arg:TrafficBuilderArgument) -> MessageBarrier
    {
        let mut tasks=None;
        let mut traffic = None;
        let mut messages_per_task_to_wait = None;
        let mut expected_messages_to_consume_to_wait = None;
        match_object_panic!(arg.cv,"MessageBarrier",value,
			"traffic" => traffic=Some(new_traffic(TrafficBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
			"tasks" | "servers" => tasks=Some(value.as_usize().expect("bad value for tasks")),
			"messages_per_task_to_wait" => messages_per_task_to_wait=Some(value.as_usize().expect("bad value for messages_per_task_to_wait")),
			"expected_messages_to_consume_to_wait" => expected_messages_to_consume_to_wait=Some(value.as_usize().expect("bad value for expected_messages_to_consume_to_wait")),
		);
        let tasks=tasks.expect("There were no tasks");
        let traffic=traffic.expect("There were no traffic");
        let messages_per_task_to_wait=messages_per_task_to_wait.expect("There were no messages_per_task_to_wait");

        if traffic.number_tasks() != tasks {
            panic!("The number of tasks in the traffic and the number of tasks in the barrier are different.");
        }

        MessageBarrier {
            tasks,
            traffic,
            messages_per_task_to_wait,
            total_sent_per_task: vec![0; tasks],
            total_sent: 0,
            total_consumed: 0,
            total_consumed_per_task: vec![0; tasks],
            expected_messages_to_consume_to_wait,
        }
    }
}

/**
MPI collectives implementations based on TrafficCredit

```ignore
Allreduce{

}

Allgather{

}

ScatterReduce{

}

All2All{

}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct MPICollective {}

impl MPICollective
{
    pub fn new(traffic: String, mut arg:TrafficBuilderArgument) ->  Box<dyn Traffic>
    {
        let traffic_cv = match traffic.as_str() {
            "ScatterReduce" =>{
                let mut tasks = None;
                let mut data_size = None;
                let mut algorithm = "Hypercube";
                match_object_panic!(arg.cv,"ScatterReduce",value,
					"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
					"algorithm" => algorithm = value.as_str().expect("bad value for algorithm"),
					"data_size" => data_size = Some(value.as_f64().expect("bad value for data_size") as usize),
				);
                match algorithm {
                    "Hypercube" => Some(get_scatter_reduce_hypercube(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"))),
                    "Ring" => Some(ring_iteration(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"))),
                    _ => panic!("Unknown algorithm: {}", algorithm),
                }
            },
            "AllGather" =>{
                let mut tasks = None;
                let mut data_size = None;
                let mut algorithm = "Hypercube";
                let mut neighbours_order = None;
                match_object_panic!(arg.cv,"AllGather",value,
					"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
					"algorithm" => algorithm = value.as_str().expect("bad value for algorithm"),
					"data_size" => data_size = Some(value.as_f64().expect("bad value for data_size") as usize),
					"neighbours_order" => neighbours_order = Some(value),
				);
                match algorithm {
                    "Hypercube" => Some(get_all_gather_hypercube(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"), neighbours_order)),
                    "Ring" => Some(ring_iteration(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"))),
                    _ => panic!("Unknown algorithm: {}", algorithm),
                }
            },
            "AllReduce" =>{
                let mut tasks = None;
                let mut data_size = None;
                let mut algorithm = "Optimal";
                let mut neighbours_order = None;
                match_object_panic!(arg.cv,"AllReduce",value,
					"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
					"algorithm" => algorithm = value.as_str().expect("bad value for algorithm"),
					"data_size" => data_size = Some(value.as_f64().expect("bad value for data_size") as usize),
					"all_gather_neighbours_order" => neighbours_order = Some(value),
				);

                match algorithm {
                    "Optimal" => Some(get_all_reduce_optimal(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"), neighbours_order)),
                    "Ring" => Some(get_all_reduce_ring(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"))),
                    _ => panic!("Unknown algorithm: {}", algorithm),
                }
            },
            "All2All" =>{
                let mut tasks = None;
                let mut data_size = None;
                match_object_panic!(arg.cv,"All2All",value,
					"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
					"data_size" => data_size = Some(value.as_f64().expect("bad value for data_size") as usize),
				);

                Some(get_all2all(tasks.expect("There were no tasks"), data_size.expect("There were no data_size")))
            },

            _ => panic!("Unknown traffic type: {}", traffic),
        };

        new_traffic(TrafficBuilderArgument{cv:&traffic_cv.expect("There should be a CV"),rng:&mut arg.rng,..arg})
    }
}

//Scater-reduce or all-gather in a ring
fn ring_iteration(tasks: usize, data_size: usize) -> ConfigurationValue {
    let message_size = ConfigurationValue::Number((data_size/tasks) as f64);
    let traffic_credit = ConfigurationValue::Object("TrafficCredit".to_string(), vec![
        ("pattern".to_string(), ConfigurationValue::Object("CartesianTransform".to_string(), vec![("sides".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(tasks as f64)])), ("shift".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(1f64)]))])),
        ("tasks".to_string(), ConfigurationValue::Number(tasks as f64)),
        ("credits_to_activate".to_string(),  ConfigurationValue::Number(1f64)),
        ("credits_per_received_message".to_string(), ConfigurationValue::Number(1f64)),
        ("messages_per_transition".to_string(), ConfigurationValue::Number(1f64)),
        ("message_size".to_string(), message_size),
        ("initial_credits".to_string() , ConfigurationValue::Object("CandidatesSelection".to_string(), vec![
            ("pattern".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
            ("pattern_destination_size".to_string(), ConfigurationValue::Number(tasks as f64)),
        ])),
    ]);

    let traffic_message_cv_builder = BuildMessageCVArgs{
        traffic: traffic_credit,
        tasks,
        messages_per_task: Some(tasks),
        num_messages: tasks * (tasks - 1),
        expected_messages_to_consume_per_task: Some(tasks),
    };

    build_message_cv(traffic_message_cv_builder)
}


fn get_scatter_reduce_hypercube(tasks: usize, data_size: usize) -> ConfigurationValue
{
    //log2 the number of tasks and panic if its not a power of 2
    let messages = (tasks as f64).log2().round() as usize;
    if 2usize.pow(messages as u32) != tasks
    {
        panic!("The number of tasks must be a power of 2");
    }
    //Now list dividing the data size by to in each iteration till number of messages
    let messages_size = ConfigurationValue::Array((1..=messages).map(|i| ConfigurationValue::Number((data_size / 2usize.pow(i as u32)) as f64)).collect::<Vec<_>>());
    let inmediate_sequence_pattern = ConfigurationValue::Object("InmediateSequencePattern".to_string(), vec![
        ("sequence".to_string(), messages_size),
    ]);

    let traffic_credit = ConfigurationValue::Object("TrafficCredit".to_string(), vec![
        ("pattern".to_string(), ConfigurationValue::Object("RecursiveDistanceHalving".to_string(), vec![])),
        ("tasks".to_string(), ConfigurationValue::Number(tasks as f64)),
        ("credits_to_activate".to_string(),  ConfigurationValue::Number(1f64)),
        ("credits_per_received_message".to_string(), ConfigurationValue::Number(1f64)),
        ("messages_per_transition".to_string(), ConfigurationValue::Number(1f64)),
        ("message_size".to_string(), ConfigurationValue::Number(0f64)),
        ("message_size_pattern".to_string(), inmediate_sequence_pattern),
        ("initial_credits".to_string() , ConfigurationValue::Object("CandidatesSelection".to_string(), vec![
            ("pattern".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
            ("pattern_destination_size".to_string(), ConfigurationValue::Number(tasks as f64)),
        ])),
    ]);

    let traffic_message_cv_builder = BuildMessageCVArgs{
        traffic: traffic_credit,
        tasks,
        messages_per_task: Some(messages),
        num_messages: messages * tasks,
        expected_messages_to_consume_per_task: Some(messages),
    };

    build_message_cv(traffic_message_cv_builder)
}

fn get_all_gather_hypercube(tasks: usize, data_size: usize, neighbours_order: Option<&ConfigurationValue>) -> ConfigurationValue
{
    //log2 the number of tasks and panic if its not a power of 2
    let messages = (tasks as f64).log2().round() as usize;
    if 2usize.pow(messages as u32) != tasks
    {
        panic!("The number of tasks must be a power of 2");
    }
    //Now list dividing the data size by to in each iteration till number of messages
    let messages_size = ConfigurationValue::Array((1..=messages).map(|i| ConfigurationValue::Number((data_size / 2usize.pow(i as u32)) as f64)).rev().collect::<Vec<_>>()); //reverse the order, starting from the smallest message to the maximum
    let inmediate_sequence_pattern = ConfigurationValue::Object("InmediateSequencePattern".to_string(), vec![
        ("sequence".to_string(), messages_size),
    ]);

    let pattern = if let Some(neighbours_order) = neighbours_order {
        ConfigurationValue::Object("RecursiveDistanceHalving".to_string(), vec![
            ("neighbours_order".to_string(), neighbours_order.clone())
        ])
    } else {
        ConfigurationValue::Object("RecursiveDistanceHalving".to_string(), vec![])
    };

    let traffic_credit = ConfigurationValue::Object("TrafficCredit".to_string(), vec![
        ("pattern".parse().unwrap(), pattern),
        ("tasks".to_string(), ConfigurationValue::Number(tasks as f64)),
        ("credits_to_activate".to_string(),  ConfigurationValue::Number(1f64)),
        ("credits_per_received_message".to_string(), ConfigurationValue::Number(1f64)),
        ("messages_per_transition".to_string(), ConfigurationValue::Number(1f64)),
        ("message_size".to_string(), ConfigurationValue::Number(0f64)),
        ("message_size_pattern".to_string(), inmediate_sequence_pattern),
        ("initial_credits".to_string() , ConfigurationValue::Object("CandidatesSelection".to_string(), vec![
            ("pattern".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
            ("pattern_destination_size".to_string(), ConfigurationValue::Number(tasks as f64)),
        ])),
    ]);

    let traffic_message_cv_builder = BuildMessageCVArgs{
        traffic: traffic_credit,
        tasks,
        messages_per_task: Some(messages),
        num_messages: messages * tasks,
        expected_messages_to_consume_per_task: Some(messages),
    };

    build_message_cv(traffic_message_cv_builder)
}

fn sum_traffics_messages(traffics: ConfigurationValue, tasks:usize, messages_to_send_per_traffic: Vec<usize>, messages_to_consume_per_traffic: Vec<usize>) -> ConfigurationValue
{
    ConfigurationValue::Object("MessageTaskSequence".to_string(), vec![
        ("tasks".to_string(), ConfigurationValue::Number(tasks as f64)),
        ("traffics".to_string(), traffics),
        ("messages_to_send_per_traffic".to_string(), ConfigurationValue::Array(messages_to_send_per_traffic.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect())),
        ("messages_to_consume_per_traffic".to_string(), ConfigurationValue::Array(messages_to_consume_per_traffic.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect())),
    ])
}

fn get_all_reduce_optimal(tasks: usize, data_size: usize, neighbours_order: Option<&ConfigurationValue>) -> ConfigurationValue
{
    let scatter_reduce = get_scatter_reduce_hypercube(tasks, data_size);
    let all_gather = get_all_gather_hypercube(tasks, data_size, neighbours_order);
    let traffic_list = ConfigurationValue::Array(vec![scatter_reduce, all_gather]);
    let messages_per_task = (tasks as f64).log2().round() as usize;
    sum_traffics_messages(traffic_list, tasks,vec![messages_per_task, messages_per_task], vec![messages_per_task, messages_per_task])
}

fn get_all_reduce_ring(tasks: usize, data_size: usize) -> ConfigurationValue
{
    let scatter_reduce = ring_iteration(tasks, data_size);
    let all_gather = ring_iteration(tasks, data_size);
    let traffic_list = ConfigurationValue::Array(vec![scatter_reduce, all_gather]);
    let messages_per_task = tasks - 1;
    sum_traffics_messages(traffic_list, tasks,vec![messages_per_task, messages_per_task], vec![messages_per_task, messages_per_task])
}

fn get_all2all(tasks: usize, data_size: usize) -> ConfigurationValue
{
    let messages = tasks -1;
    let message_size = ConfigurationValue::Number((data_size/tasks) as f64);
    let traffic_credit = ConfigurationValue::Object("TrafficCredit".to_string(), vec![
        ("pattern".to_string(), ConfigurationValue::Object("ElementComposition".to_string(), vec![
            ("pattern".to_string(), ConfigurationValue::Object("CartesianTransform".to_string(), vec![
                ("sides".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(tasks as f64)])),
                ("shift".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(1f64)]))
            ]))
        ])),
        ("tasks".to_string(), ConfigurationValue::Number(tasks as f64)),
        ("credits_to_activate".to_string(),  ConfigurationValue::Number(1f64)),
        ("credits_per_received_message".to_string(), ConfigurationValue::Number(0f64)),
        ("messages_per_transition".to_string(), ConfigurationValue::Number(messages as f64)),
        ("message_size".to_string(), message_size),
        ("initial_credits".to_string() , ConfigurationValue::Object("CandidatesSelection".to_string(), vec![
            ("pattern".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
            ("pattern_destination_size".to_string(), ConfigurationValue::Number(tasks as f64)),
        ])),
    ]);

    let traffic_message_cv_builder = BuildMessageCVArgs{
        traffic: traffic_credit,
        tasks,
        messages_per_task: Some(messages),
        num_messages: tasks * messages,
        expected_messages_to_consume_per_task: Some(messages),
    };

    build_message_cv(traffic_message_cv_builder)
}

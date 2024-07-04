use crate::pattern::extra::get_cartesian_transform;
use crate::pattern::extra::get_candidates_selection;
use crate::AsMessage;
use std::rc::Rc;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use crate::config_parser::ConfigurationValue;
use crate::{match_object_panic, Message, Time};
use crate::topology::Topology;
use crate::traffic::{new_traffic, TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};
use crate::traffic::basic::{build_message_cv, BuildMessageCVArgs};
use crate::traffic::mini_apps::{BuildTrafficCreditCVArgs, get_traffic_credit};
use crate::traffic::sequences::{BuilderMessageTaskSequenceCVArgs, get_traffic_message_task_sequence};
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

    fn should_generate(&mut self, task:usize, cycle:Time, rng: &mut StdRng) -> bool
    {
        self.total_sent_per_task[task] < self.messages_per_task_to_wait && self.traffic.should_generate(task, cycle, rng)
    }

    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        self.total_consumed += 1;
        self.total_consumed_per_task[task] += 1;
        if self.total_sent == self.total_consumed && self.messages_per_task_to_wait * self.tasks == self.total_sent {
            self.total_sent = 0;
            self.total_consumed = 0;
            self.total_sent_per_task = vec![0; self.tasks];
            self.total_consumed_per_task = vec![0; self.tasks];
        }
        self.traffic.consume(task, message, cycle, topology, rng)
    }
    fn is_finished(&self) -> bool
    {
        self.traffic.is_finished()
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

pub struct BuildMessageBarrierCVArgs {
    pub traffic: ConfigurationValue,
    pub tasks: usize,
    pub messages_per_task_to_wait: usize,
    pub expected_messages_to_consume_to_wait: Option<usize>,
}

pub fn build_message_barrier_cv(args: BuildMessageBarrierCVArgs) -> ConfigurationValue
{
    let mut cv = vec![
        ("traffic".to_string(), args.traffic),
        ("tasks".to_string(), ConfigurationValue::Number(args.tasks as f64)),
        ("messages_per_task_to_wait".to_string(), ConfigurationValue::Number(args.messages_per_task_to_wait as f64)),
    ];

    if let Some(expected_messages_to_consume_to_wait) = args.expected_messages_to_consume_to_wait {
        cv.push(("expected_messages_to_consume_to_wait".to_string(), ConfigurationValue::Number(expected_messages_to_consume_to_wait as f64)));
    }

    ConfigurationValue::Object("MessageBarrier".to_string(), cv)
}


/**
MPI collectives implementations based on TrafficCredit

```ignore
Allgather{
    tasks: 64,
    data_size: 1000, //The total data size to all-gather. Each task starts with a data slice of size data_size/tasks.
    algorithm: "Hypercube",
    neighbours_order: [32, 16, 8, 4, 2, 1], //Optional, the order to iter hypercube neighbours
}

ScatterReduce{
    tasks: 64,
    data_size: 1000, //The total data size to scatter-reduce. Each task ends with a data slice reduced of size data_size/tasks.
    algorithm: "Hypercube",
}

Allreduce{
    tasks: 64,
    data_size: 1000, //The total data size to all-reduce.
    algorithm: "Optimal",
    all_gather_neighbours_order: [32, 16, 8, 4, 2, 1], //Optional, the order to iter hypercube neighbours in the all-gather
}

All2All{
    tasks: 64,
    data_size: 1000, //The total data size to all2all. Each task sends a data slice of size data_size/tasks to all the other tasks.
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

    let message_size = data_size/tasks;

    let candidates_selection = get_candidates_selection(
        ConfigurationValue::Object("Identity".to_string(), vec![]),
        tasks,
    );

    let pattern_cartesian_transform = get_cartesian_transform(
    vec![tasks],
        Some(vec![1]),
        None,
    );

    let traffic_credit_args = BuildTrafficCreditCVArgs{
        tasks,
        credits_to_activate: 1,
        credits_per_received_message: 1,
        messages_per_transition: 1,
        message_size,
        pattern: pattern_cartesian_transform,
        initial_credits: candidates_selection,
        message_size_pattern: None,
    };

    let traffic_credit = get_traffic_credit(traffic_credit_args);

    let traffic_message_cv_builder = BuildMessageCVArgs{
        traffic: traffic_credit,
        tasks,
        messages_per_task: Some(tasks -1),
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

    let candidates_selection = get_candidates_selection(
        ConfigurationValue::Object("Identity".to_string(), vec![]),
        tasks,
    );

    let pattern_distance_halving = ConfigurationValue::Object("RecursiveDistanceHalving".to_string(), vec![]);

    let traffic_credit_args = BuildTrafficCreditCVArgs{
        tasks,
        credits_to_activate: 1,
        credits_per_received_message: 1,
        messages_per_transition: 1,
        message_size: 0,
        pattern: pattern_distance_halving,
        initial_credits: candidates_selection,
        message_size_pattern: Some(inmediate_sequence_pattern),
    };

    let traffic_credit = get_traffic_credit(traffic_credit_args);

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


    let candidates_selection = get_candidates_selection(
        ConfigurationValue::Object("Identity".to_string(), vec![]),
        tasks,
    );

    let pattern_distance_halving = if let Some(neighbours_order) = neighbours_order {
        ConfigurationValue::Object("RecursiveDistanceHalving".to_string(), vec![
            ("neighbours_order".to_string(), neighbours_order.clone())
        ])
    } else {
        ConfigurationValue::Object("RecursiveDistanceHalving".to_string(), vec![])
    };

    let traffic_credit_args = BuildTrafficCreditCVArgs{
        tasks,
        credits_to_activate: 1,
        credits_per_received_message: 1,
        messages_per_transition: 1,
        message_size: 0,
        pattern: pattern_distance_halving,
        initial_credits: candidates_selection,
        message_size_pattern: Some(inmediate_sequence_pattern),
    };

    let traffic_credit = get_traffic_credit(traffic_credit_args);

    let traffic_message_cv_builder = BuildMessageCVArgs{
        traffic: traffic_credit,
        tasks,
        messages_per_task: Some(messages),
        num_messages: messages * tasks,
        expected_messages_to_consume_per_task: Some(messages),
    };

    build_message_cv(traffic_message_cv_builder)
}

fn get_all_reduce_optimal(tasks: usize, data_size: usize, neighbours_order: Option<&ConfigurationValue>) -> ConfigurationValue
{
    let scatter_reduce_hypercube = get_scatter_reduce_hypercube(tasks, data_size);
    let all_gather_hypercube = get_all_gather_hypercube(tasks, data_size, neighbours_order);

    let messages_per_task = (tasks as f64).log2().round() as usize;
    let traffic_message_task_sequence_args = BuilderMessageTaskSequenceCVArgs{
        tasks,
        traffics: vec![scatter_reduce_hypercube, all_gather_hypercube],
        messages_to_send_per_traffic: vec![messages_per_task, messages_per_task],
        messages_to_consume_per_traffic: Some(vec![messages_per_task, messages_per_task]),
    };
    get_traffic_message_task_sequence(traffic_message_task_sequence_args)
}

fn get_all_reduce_ring(tasks: usize, data_size: usize) -> ConfigurationValue
{
    let scatter_reduce_ring = ring_iteration(tasks, data_size);
    let all_gather_ring = ring_iteration(tasks, data_size);
    let messages_per_task = tasks - 1;

    let traffic_message_task_sequence_args = BuilderMessageTaskSequenceCVArgs{
        tasks,
        traffics: vec![scatter_reduce_ring, all_gather_ring],
        messages_to_send_per_traffic: vec![messages_per_task, messages_per_task],
        messages_to_consume_per_traffic: Some(vec![messages_per_task, messages_per_task]),
    };

    get_traffic_message_task_sequence(traffic_message_task_sequence_args)
}

fn get_all2all(tasks: usize, data_size: usize) -> ConfigurationValue
{
    let messages = tasks -1;
    let message_size = data_size/tasks;

    let candidates_selection = get_candidates_selection(
        ConfigurationValue::Object("Identity".to_string(), vec![]),
        tasks,
    );

    let pattern_cartesian_transform = get_cartesian_transform(
        vec![tasks],
        Some(vec![1]),
        None,
    );

    let element_composition = ConfigurationValue::Object("ElementComposition".to_string(), vec![
        ("pattern".to_string(), pattern_cartesian_transform),
    ]);

    let traffic_credit_args = BuildTrafficCreditCVArgs{
        tasks,
        credits_to_activate: 1,
        credits_per_received_message: 0,
        messages_per_transition: messages,
        message_size,
        pattern: element_composition,
        initial_credits: candidates_selection,
        message_size_pattern: None,
    };

    let traffic_credit = get_traffic_credit(traffic_credit_args);

    let traffic_message_cv_builder = BuildMessageCVArgs{
        traffic: traffic_credit,
        tasks,
        messages_per_task: Some(messages),
        num_messages: tasks * messages,
        expected_messages_to_consume_per_task: Some(messages),
    };

    build_message_cv(traffic_message_cv_builder)
}

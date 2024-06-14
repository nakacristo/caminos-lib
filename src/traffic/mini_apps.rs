use crate::pattern::{new_pattern, PatternBuilderArgument};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use crate::config_parser::ConfigurationValue;
use crate::{match_object_panic, Message, Time};
use crate::pattern::{get_candidates_selection, get_cartesian_transform, get_hotspot_destination, get_switch_pattern, Pattern};
use crate::topology::Topology;
use crate::traffic::{new_traffic, TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};
use crate::traffic::basic::{build_message_cv, BuildMessageCVArgs};
use crate::traffic::TaskTrafficState::{Generating, UnspecifiedWait};


/**
Traffic which allow tasks to generate messages when they have enough credits.
After generating the messages, the credits are consumed.
A task gain credits when it consumes messages, and an initial amount of credits per task can be set.
```ignore
TrafficCredit{
	pattern: RandomPermutation, //specify the pattern of the communication
	tasks: 1000, //specify the number of tasks
	credits_to_activate: 10, //specify the number of credits needed to generate messages
	messages_per_transition: 1, //specify the number of messages each task can sent when consuming credits
	credits_per_received_message: 1, //specify the number of credits to gain when a message is received
	message_size: 16, //specify the size of each sent message
	message_size_pattern: Hotspots{destinatinos:[128]} //Variable message size to add depending on the task
	initial_credits: Hotspots{destinations: [1]}, //specify the initial amount of credits per task
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct TrafficCredit
{
	///Number of tasks applying this traffic.
	tasks: usize,
	///The pattern of the communication.
	pattern: Box<dyn Pattern>,
	///Credits needed to activate the transition
	credits_to_activate:usize,
	///Credit count per origin
	credits: Vec<usize>,
	///The credits to sum when a message is received
	credits_per_received_message:usize,
	///The size of each sent message.
	message_size: usize,
	///Pattern message size, variable
	message_size_pattern: Option<Box<dyn Pattern>>,
	///Messages per transition
	messages_per_transition:usize,
	///The number of messages each task has pending to sent.
	pending_messages: Vec<usize>,
	///Set of generated messages.
	generated_messages: BTreeSet<*const Message>,
}

impl Traffic for TrafficCredit
{
	fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
	{
		if origin>=self.tasks
		{
			panic!("origin {} does not belong to the traffic",origin);
			// return Err(TrafficError::OriginOutsideTraffic);
		}
		if self.pending_messages[origin] == 0
		{
			panic!("origin {} has no pending messages",origin);
		}
		self.pending_messages[origin]-=1;
		let destination=self.pattern.get_destination(origin, topology, rng);
		if origin==destination
		{
			return Err(TrafficError::SelfMessage);
		}
		let message_size = self.message_size + if let Some(patron) = self.message_size_pattern.as_ref(){
			patron.get_destination(origin,topology,rng)
		}else{
			0
		};
		let message=Rc::new(Message{
			origin,
			destination,
			size: message_size,
			creation_cycle: cycle,
			cycle_into_network: RefCell::new(None),
		});
		self.generated_messages.insert(message.as_ref() as *const Message);
		Ok(message)
	}
	fn probability_per_cycle(&self, task:usize) -> f32
	{
		if self.pending_messages[task]>0
		{
			1.0
		}
		else
		{
			0.0
		}
	}

	fn should_generate(self: &mut TrafficCredit, task:usize, _cycle:Time, _rng: &mut StdRng) -> bool
	{
		while self.credits[task] >= self.credits_to_activate
		{
			self.pending_messages[task] += self.messages_per_transition;
			self.credits[task] -= self.credits_to_activate;
		}
		self.pending_messages[task] > 0
	}

	fn try_consume(&mut self, task:usize, message: Rc<Message>, _cycle:Time, _topology:&dyn Topology, _rng: &mut StdRng) -> bool
	{
		let message_ptr=message.as_ref() as *const Message;
		self.credits[task] += self.credits_per_received_message;
		self.generated_messages.remove(&message_ptr)
	}
	fn is_finished(&self) -> bool
	{
		if !self.generated_messages.is_empty() //messages traveling through the network
		{
			return false;
		}

		if self.pending_messages.iter().sum::<usize>() > 0 //messages waiting to be sent
		{
			return false;
		}

		//if there is a task with enough credits to activate, then it is not finished
		for &c in self.credits.iter()
		{
			if c >= self.credits_to_activate
			{
				return false;
			}
		}
		true
	}
	fn task_state(&self, task:usize, _cycle:Time) -> Option<TaskTrafficState>
	{
		if self.pending_messages[task]>0 {
			Some(Generating)
		} else {
			//We do not know whether someone is sending us data.
			//if self.is_finished() { TaskTrafficState::Finished } else { TaskTrafficState::UnspecifiedWait }
			// Sometimes it could be Finished, but it is not worth computing...
			Some(UnspecifiedWait)
		}
	}

	fn number_tasks(&self) -> usize {
		self.tasks
	}
}

impl TrafficCredit
{
	pub fn new(arg:TrafficBuilderArgument) -> TrafficCredit
	{
		let mut tasks=None;
		let mut pattern =None;
		let mut credits_to_activate=None;
		let mut credits_per_received_message=None;
		let mut message_size=None;
		let mut messages_per_transition=None;
		let mut initial_credits=None;
		let mut message_size_pattern = None;

		match_object_panic!(arg.cv,"TrafficCredit",value,
			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"tasks" | "servers" => tasks=Some(value.as_usize().expect("bad value for tasks")),
			"credits_to_activate" => credits_to_activate=Some(value.as_usize().expect("bad value for credits_to_activate")),
			"credits_per_received_message" => credits_per_received_message=Some(value.as_usize().expect("bad value for credits_per_received_message")),
			"message_size" => message_size=Some(value.as_usize().expect("bad value for message_size") ),
			"messages_per_transition" => messages_per_transition=Some(value.as_usize().expect("bad value for messages_per_transition")),
			"initial_credits" => initial_credits=Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"message_size_pattern" => message_size_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
		);

		let tasks=tasks.expect("There were no tasks");
		let mut pattern = pattern.expect("There were no pattern");
		let credits_to_activate=credits_to_activate.expect("There were no credits_to_activate");
		let credits_per_received_message=credits_per_received_message.expect("There were no credits_per_received_message");
		let message_size=message_size.expect("There were no message_size");
		let messages_per_transition=messages_per_transition.expect("There were no messages_per_transition");
		let mut initial_credits=initial_credits.expect("There were no initial_credits");

		pattern.initialize(tasks, tasks, arg.topology, arg.rng);
		initial_credits.initialize(tasks, tasks, arg.topology, arg.rng);
		let pending_messages = vec![0;tasks];

		let credits = (0..tasks).map(|i| initial_credits.get_destination(i, arg.topology, arg.rng)).collect::<Vec<usize>>();

		if let Some(patron) = message_size_pattern.as_mut(){
			patron.initialize(tasks, tasks, arg.topology, arg.rng);
		}
		TrafficCredit{
			tasks,
			pattern,
			credits_to_activate,
			credits,
			credits_per_received_message,
			message_size,
			message_size_pattern,
			messages_per_transition,
			pending_messages,
			generated_messages: BTreeSet::new(),
		}
	}
}

pub struct BuildTrafficCreditCVArgs{
	pub tasks: usize,
	pub credits_to_activate:usize,
	pub messages_per_transition: usize,
	pub credits_per_received_message: usize,
	pub message_size: usize,
	pub pattern: ConfigurationValue,
	pub initial_credits: ConfigurationValue,
	pub message_size_pattern: Option<ConfigurationValue>
}


pub fn get_traffic_credit(args: BuildTrafficCreditCVArgs) -> ConfigurationValue
{
	let mut arg_vec = vec![
		("tasks".to_string(), ConfigurationValue::Number(args.tasks as f64)),
		("credits_to_activate".to_string(), ConfigurationValue::Number(args.credits_to_activate as f64)),
		("messages_per_transition".to_string(), ConfigurationValue::Number(args.messages_per_transition as f64)),
		("credits_per_received_message".to_string(), ConfigurationValue::Number(args.credits_per_received_message as f64)),
		("message_size".to_string(), ConfigurationValue::Number(args.message_size as f64)),
		("pattern".to_string(), args.pattern),
		("initial_credits".to_string(), args.initial_credits),
	];

	if let Some(message_size_pattern) = args.message_size_pattern {
		arg_vec.push(("message_size_pattern".to_string(), message_size_pattern));
	}

	ConfigurationValue::Object("TrafficCredit".to_string(), arg_vec)
}



#[derive(Quantifiable)]
#[derive(Debug)]
pub struct MiniApp {}

impl MiniApp {

    pub fn new(traffic: String, arg:TrafficBuilderArgument) -> Box<dyn Traffic> {

        let traffic_cv = match traffic.as_str() {

            "Wavefront" => {
                let mut task_space = None;
                let mut data_size = None;
                let mut num_messages = None;

                match_object_panic!(arg.cv, "Wavefront", value,
                    "task_space" => task_space = Some(value.as_array().expect("Bad task_space value").iter().map(|v| v.as_f64().expect("Bad task_space value") as usize).collect()),
                    "data_size" => data_size = Some(value.as_f64().expect("Bad data_size value") as usize),
                    "num_messages" => num_messages = Some(value.as_f64().expect("Bad num_messages value") as usize),
                );

                let task_space = task_space.expect("task_space is required");
                let data_size = data_size.expect("data_size is required");
                let num_messages = num_messages.expect("num_messages is required");

                get_wavefront(task_space, data_size, num_messages)
            },
            _ => panic!("Unknown traffic type: {}", traffic),
        };
        new_traffic(TrafficBuilderArgument{cv: &traffic_cv, ..arg})
    }

}

fn get_wavefront(task_space: Vec<usize>, data_size:usize, num_messages: usize) -> ConfigurationValue{
	let tasks = task_space.iter().product();
	let _task_space_cv: Vec<_> = task_space.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect();

	let identity_pattern_vector = vec![ConfigurationValue::Object("Identity".to_string(), vec![]); task_space.len()];

	let initial_credits = ConfigurationValue::Object("Sum".to_string(), vec![
		("patterns".to_string(), ConfigurationValue::Array(
			(0..task_space.len()).into_iter().enumerate().map(|(i, _z)| {
					let mut patterns_cartesian_transform = identity_pattern_vector.clone();
					patterns_cartesian_transform[i] = get_hotspot_destination(vec![0]); //ConfigurationValue::Object("Hotspots".to_string(), vec![("destinations".to_string(),ConfigurationValue::Array())]);
					let pattern_cad_sel = get_cartesian_transform(task_space.clone(), None, Some(patterns_cartesian_transform));
				  get_candidates_selection(pattern_cad_sel, tasks)
			}).collect())
		),
		("middle_sizes".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(2f64); task_space.len()])),
	]);

	let traffic_credit_pattern = (0..task_space.len()).into_iter().map(|i|
		 {
			 let mut patterns = identity_pattern_vector.clone();
			 patterns[i]= get_hotspot_destination(vec![task_space[i] -1]);
			 let cartesian_transform = get_cartesian_transform(task_space.clone(), None, Some(patterns));
			 let switch_indexing = get_candidates_selection(cartesian_transform, tasks);
			 let mut shift = vec![0; task_space.len()];
			 shift[i] = 1;
			 let switch_patterns = vec![
				 get_cartesian_transform(task_space.clone(), Some(shift), None),
				 ConfigurationValue::Object("Identity".to_string(), vec![]), //Its in a edge of the n-dimensional space
			 ];
			 get_switch_pattern(switch_indexing, switch_patterns)
		 }
	).collect();
	let traffic_credit_pattern = ConfigurationValue::Object( "RoundRobin".to_string(), vec![
		("patterns".to_string(), ConfigurationValue::Array(traffic_credit_pattern)),
	]);

	let traffic_credit_params = BuildTrafficCreditCVArgs {
		tasks,
		credits_to_activate: task_space.len(),
		messages_per_transition: task_space.len(),
		message_size: data_size,
		credits_per_received_message: 1,
		pattern: traffic_credit_pattern,
		initial_credits,
		message_size_pattern: None
	};

	let traffic_credit = get_traffic_credit(traffic_credit_params);

	let traffic_message_cv_builder = BuildMessageCVArgs{
		traffic: traffic_credit.clone(),
		tasks,
		num_messages,
		messages_per_task: None,
		expected_messages_to_consume_per_task: Some(task_space.len()),
	};

	build_message_cv(traffic_message_cv_builder)
}
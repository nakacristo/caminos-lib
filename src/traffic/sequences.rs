use crate::AsMessage;
use crate::pattern::{new_pattern, PatternBuilderArgument};
use std::collections::{BTreeSet};
use std::convert::TryInto;
use std::rc::Rc;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use crate::{match_object_panic, Message, Time};
use crate::config_parser::ConfigurationValue;
use crate::measures::TrafficStatistics;
use crate::packet::ReferredPayload;
use crate::pattern::Pattern;
use crate::topology::Topology;
use crate::traffic::{new_traffic, TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};
use crate::traffic::TaskTrafficState::{Finished, FinishedGenerating, UnspecifiedWait, WaitingCycle};

/**
A sequence of traffics. When a traffic declares itself to be finished moves to the next.

All the subtraffics in `traffics` must give the same value for `number_tasks`, which is also used for Sequence. At least one such subtraffic must be provided.

```ignore
Sequence{
	traffics: [Burst{...}, Burst{...}],
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Sequence
{
	///List of applicable traffics.
	traffics: Vec<Box<dyn Traffic>>,
	//How many times to apply the whole traffic period. default to 1.
	//period_limit: usize,
	///The traffic which is currently in use.
	current_traffic: usize,
	//The period number, starting at 0. The whole traffic finishes before `current_period` reaching `period_limit`.
	//current_period: usize,
}

impl Traffic for Sequence
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        // while self.traffics[self.current_traffic].is_finished()
        // {
        //     self.current_traffic += 1;
        //     //self.current_traffic = (self.current_traffic + 1) % self.traffics.len();
        // }
        assert!(self.current_traffic<=self.traffics.len());
        self.traffics[self.current_traffic].generate_message(origin,cycle,topology,rng)
    }
    fn probability_per_cycle(&self,task:usize) -> f32
    {
        if self.current_traffic < self.traffics.len() {
            self.traffics[self.current_traffic].probability_per_cycle(task)
        }else{
            0.0
        }
    }
    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        self.traffics[self.current_traffic].consume(task, message, cycle, topology, rng)
    }
    fn is_finished(&self) -> bool
    {
        return self.current_traffic>=self.traffics.len() || (self.current_traffic==self.traffics.len()-1 && self.traffics[self.current_traffic].is_finished())
    }
    fn should_generate(&mut self, task:usize, cycle:Time, rng: &mut StdRng) -> bool
    {
        if self.current_traffic>=self.traffics.len()
        {
            return false;
        }

        while self.current_traffic < self.traffics.len() && self.traffics[self.current_traffic].is_finished()
        {
            self.current_traffic += 1;
        }

        if self.current_traffic>=self.traffics.len()
        {
            false
        } else {
            self.traffics[self.current_traffic].should_generate(task,cycle,rng)
        }
    }
    fn task_state(&self, task:usize, cycle:Time) -> Option<TaskTrafficState>
    {
        use crate::traffic::TaskTrafficState::*;
        if self.current_traffic>=self.traffics.len()
        {
            Some(Finished)
        } else {
            let state = self.traffics[self.current_traffic].task_state(task,cycle).expect("TODO! the none case");
            if let Finished=state{
                Some(UnspecifiedWait)
            } else {
                Some(state)
            }
            //In the last traffic we could try to check for FinishedGenerating
        }
    }

    fn number_tasks(&self) -> usize {
        // every traffic has the same number of tasks
        self.traffics[0].number_tasks()
    }
}

impl Sequence
{
	pub fn new(arg:TrafficBuilderArgument) -> Sequence
	{
		let mut traffics_args =None;
		let mut period_number=1usize;
		match_object_panic!(arg.cv,"Sequence",value,
			"traffics" => traffics_args = Some(value.as_array().expect("bad value for traffics")),
			"period_number" => period_number=value.as_f64().expect("bad value for period_number") as usize,
		);
		let traffics_args=traffics_args.expect("There were no traffics");
		let TrafficBuilderArgument{plugs,topology,rng, ..} = arg;
		let traffics : Vec<_> = (0..period_number).flat_map(|_ip| traffics_args.iter().map(
			|v|new_traffic(TrafficBuilderArgument{cv:v,plugs,topology,rng:&mut *rng})
		).collect::<Vec<_>>() ).collect();
		//let mut traffics = Vec::with_capacity(period_number*traffics_args.len());
		//for _ip in 0..period_number
		//{
		//	for v in traffics_args
		//	{
		//		//traffics.push( new_traffic(TrafficBuilderArgument{cv:v,..arg}) );
		//		traffics.push( new_traffic(TrafficBuilderArgument{cv:v,plugs,topology,rng}) );
		//	}
		//}
		assert!( !traffics.is_empty() , "Cannot make a Sequence of 0 traffics." );
		let size = traffics[0].number_tasks();
		for traffic in traffics.iter().skip(1)
		{
			assert_eq!( traffic.number_tasks(), size , "In Sequence all sub-traffics must involve the same number of tasks." );
		}
		Sequence{
			traffics,
			current_traffic:0,
			//current_period:0,
		}
	}
}

/**
A sequence of traffics. Each task independently sends/consumes a number of messages before moving to the next traffic.
```ignore
MessageTaskSequence{
    tasks: 1000,
    traffics: [Burst{...}, Burst{...}],
    messages_to_send_per_traffic: [100, 200],
    messages_to_consume_per_traffic: [100, 200], //Optional
}
```
 **/

#[derive(Quantifiable)]
#[derive(Debug)]
pub struct MessageTaskSequence
{
    ///List of applicable traffics.
    traffics: Vec<Box<dyn Traffic>>,
    ///The number of messages to send per traffic
    messages_to_send_per_traffic: Vec<usize>,
    ///The number of messages to consume per traffic
    messages_to_consume_per_traffic: Option<Vec<usize>>,
    ///The number of messages sent by each task
    messages_sent: Vec<Vec<usize>>,
    ///The number of messages consumed by each task
    messages_consumed: Vec<Vec<usize>>,
    ///Generated messages per traffic
    generated_messages: BTreeSet<u128>,
    id: u128,
}

impl Traffic for MessageTaskSequence
{
    fn generate_message(&mut self, origin: usize, cycle: Time, topology: &dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>, TrafficError> {
        let messages_sent = &mut self.messages_sent[origin];
        let messages_to_send_per_traffic = &self.messages_to_send_per_traffic;

        for i in 0..self.traffics.len() {
            if messages_sent[i] < messages_to_send_per_traffic[i] {
                let message = self.traffics[i].generate_message(origin, cycle, topology, rng)?;
                let id = self.id;
                self.generated_messages.insert(id);

                let mut payload = Vec::with_capacity(message.payload().len() + 32);
                let bytes_argument = &[id, i as u128];
                let vec_payload = bytemuck::bytes_of(bytes_argument);
                payload.extend_from_slice(&vec_payload);
                payload.extend_from_slice(message.payload());

                let message = Rc::new(Message {
                    origin,
                    destination: message.destination(),
                    size: message.size(),
                    creation_cycle: message.creation_cycle(),
                    payload,
                    id_traffic: None,
                });

                messages_sent[i] += 1;
                self.id += 1;
                return Ok(message);
            }
        }
        panic!("No more messages to send");
    }

    fn probability_per_cycle(&self, task: usize) -> f32 {
        let messages_sent = & self.messages_sent[task];
        let messages_consumed = & self.messages_consumed[task];

        for i in 0..self.traffics.len() {
            if messages_sent[i] < self.messages_to_send_per_traffic[i] {
                return 1.0;
            }else{
                if let Some(messages_to_consume_per_traffic) = &self.messages_to_consume_per_traffic {
                    if messages_consumed[i] < messages_to_consume_per_traffic[i] {
                        return 0.0;
                    }
                }
            }
        }
        0.0
    }

    fn consume(&mut self, task: usize, message: &dyn AsMessage, cycle: Time, topology: &dyn Topology, rng: &mut StdRng) -> bool {
        let messages_consumed = &mut self.messages_consumed[task];
        let [id,index ] = bytemuck::try_cast::<[u8;32],[u128;2]>(message.payload()[0..32].try_into().expect("This should be here!")).expect("MessageTaskSequence: bad payload in consume");
        let mut down_message = ReferredPayload::from(message);
        down_message.payload = &message.payload()[32..];

        if self.generated_messages.remove(&id) {
            messages_consumed[ index as usize ] += 1;

            self.traffics[ index as usize ].consume(task, &down_message, cycle, topology, rng)

        }else {
            panic!("A message was consumed that was not generated by this traffic");
        }
    }

    fn is_finished(&self) -> bool {

        if self.generated_messages.len() > 0
        {
            return false;
        }

        for i in 0..self.traffics.len() {
            if !self.messages_sent.iter().all(|messages_sent| messages_sent[i] >= self.messages_to_send_per_traffic[i])
            {
                return false;
            }
            if let Some(messages_to_consume_per_traffic) = &self.messages_to_consume_per_traffic {
                if !self.messages_consumed.iter().all(|messages_consumed| messages_consumed[i] >= messages_to_consume_per_traffic[i]) {
                    return false;
                }
            }
        }

        true
    }

    fn should_generate(&mut self, task: usize, cycle: Time, rng: &mut StdRng) -> bool {
        let messages_sent = &mut self.messages_sent[task];
        let messages_consumed = &mut self.messages_consumed[task];

        for i in 0..self.traffics.len() {
            if messages_sent[i] < self.messages_to_send_per_traffic[i] {
                return self.traffics[i].should_generate(task, cycle, rng); //Maybe true or not

            }else{
                if let Some(messages_to_consume_per_traffic) = &self.messages_to_consume_per_traffic {
                    if messages_consumed[i] < messages_to_consume_per_traffic[i] {
                        return false;
                    }
                }
            }
        }
        false
    }

    fn task_state(&self, task: usize, cycle: Time) -> Option<TaskTrafficState> {
        for i in 0..self.traffics.len(){
            if self.messages_sent[task][i] < self.messages_to_send_per_traffic[i]{
                return self.traffics[i].task_state(task, cycle);
            }else if self.messages_to_consume_per_traffic.is_some() {
                let to_consume = self.messages_to_consume_per_traffic.as_ref().unwrap();
                if self.messages_sent[task][i] < to_consume[i]{
                    return Some(UnspecifiedWait)
                }
            }
        }
        if self.messages_to_consume_per_traffic.is_some(){
            let to_consume = self.messages_to_consume_per_traffic.as_ref().unwrap();
            if self.messages_consumed[task].iter().sum::<usize>() < to_consume.iter().sum::<usize>(){
                return Some(FinishedGenerating)
            }else {
                Some(Finished)
            }
        }else{
            Some(FinishedGenerating)
        }
    }

    fn number_tasks(&self) -> usize {
        self.traffics[0].number_tasks()
    }

    fn get_statistics(&self) -> Option<TrafficStatistics> {
        None
    }
}

impl MessageTaskSequence
{
    pub fn new(arg: TrafficBuilderArgument) -> MessageTaskSequence
    {
        let mut traffics_args = None;
        let mut messages_to_send_per_traffic = None;
        let mut messages_to_consume_per_traffic = None;
        let mut tasks= None;
        match_object_panic!(arg.cv, "MessageTaskSequence", value,
			"tasks" => tasks = Some(value.as_usize().expect("Number of tasks for MessageTaskSequence wrong")),
			"traffics" => traffics_args = Some(value.as_array().expect("bad value for traffics")),
			"messages_to_send_per_traffic" => messages_to_send_per_traffic = Some(value.as_array().expect("bad value for messages_to_send_per_traffic").iter().map(|v| v.as_f64().expect("bad value in messages_to_send_per_traffic") as usize).collect()),
			"messages_to_consume_per_traffic" => messages_to_consume_per_traffic = Some(value.as_array().expect("bad value for messages_to_consume_per_traffic").iter().map(|v| v.as_f64().expect("bad value in messages_to_consume_per_traffic") as usize).collect()),
		);
        let tasks = tasks.expect("Number of tasks for MessageTaskSequence should be indicated");
        let traffics_args = traffics_args.expect("There were no traffics");
        let TrafficBuilderArgument { plugs, topology, rng, .. } = arg;
        let traffics: Vec<_> = traffics_args.iter().map(|v| new_traffic(TrafficBuilderArgument { cv: v, plugs, topology, rng: &mut *rng })).collect();
        let messages_to_send_per_traffic = messages_to_send_per_traffic.expect("There were no messages_to_send_per_traffic");
        let messages_to_consume_per_traffic = messages_to_consume_per_traffic;
        for traffic in traffics.iter()
        {
            assert_eq!(traffic.number_tasks(), tasks, "In MessageTaskSequence all sub-traffics must involve the same number of tasks.");
        }
        let traffic_len = traffics.len();
        MessageTaskSequence {
            traffics,
            messages_to_send_per_traffic,
            messages_to_consume_per_traffic,
            messages_sent: vec![ vec![0; traffic_len ]; tasks ],
            messages_consumed: vec![ vec![0; traffic_len ]; tasks ],
            generated_messages: BTreeSet::new(),
            id: 0,
        }
    }
}

pub struct BuilderMessageTaskSequenceCVArgs {
    pub tasks: usize,
    pub traffics: Vec<ConfigurationValue>,
    pub messages_to_send_per_traffic: Vec<usize>,
    pub messages_to_consume_per_traffic: Option<Vec<usize>>,
}

pub fn get_traffic_message_task_sequence(args: BuilderMessageTaskSequenceCVArgs) -> ConfigurationValue{
    let mut arg_vec = vec![
        ("tasks".to_string(), ConfigurationValue::Number(args.tasks as f64)),
        ("traffics".to_string(), ConfigurationValue::Array(args.traffics)),
        ("messages_to_send_per_traffic".to_string(), ConfigurationValue::Array(args.messages_to_send_per_traffic.iter().map(|v| ConfigurationValue::Number(*v as f64)).collect())),
    ];

    if let Some(messages_to_consume_per_traffic) = args.messages_to_consume_per_traffic {
        arg_vec.push(("messages_to_consume_per_traffic".to_string(), ConfigurationValue::Array(messages_to_consume_per_traffic.iter().map(|v| ConfigurationValue::Number(*v as f64)).collect())));
    }

    ConfigurationValue::Object("MessageTaskSequence".to_string(), arg_vec)
}


/// Like the `Burst` pattern, but generating messages from different patterns and with different message sizes.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct MultimodalBurst
{
	///Number of tasks applying this traffic.
	tasks: usize,
	/// For each kind of message `provenance` we have
	/// `(pattern,total_messages,message_size,step_size)`
	/// a Pattern deciding the destination of the message
	/// a usize with the total number of messages of this kind that each task must generate
	/// a usize with the size of each message size.
	/// a usize with the number of messages to send of this kind before switching to the next one.
	provenance: Vec< (Box<dyn Pattern>,usize,usize,usize) >,
	///For each task and kind we track `pending[task][kind]=(total_remaining,step_remaining)`.
	///where `total_remaining` is the total number of messages of this kind that this task has yet to send.
	///and `step_remaining` is the number of messages that the task will send before switch to the next kind.
	pending: Vec<Vec<(usize,usize)>>,
	///For each task we track which provenance kind is the next one.
	///If for the annotated provenance there is not anything else to send then use the next one.
	next_provenance: Vec<usize>,
	///Set of generated messages.
	generated_messages: BTreeSet<u128>,
	next_id: u128,
}

impl Traffic for MultimodalBurst
{
	fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
	{
		if origin>=self.tasks
		{
			//panic!("origin {} does not belong to the traffic",origin);
			return Err(TrafficError::OriginOutsideTraffic);
		}
		let pending = &mut self.pending[origin];
		// Determine the kind to use.
		let mut provenance_index = self.next_provenance[origin];
		loop
		{
			let (ref mut total_remaining, ref mut step_remaining) = pending[provenance_index];
			if *total_remaining > 0
			{
				*step_remaining -=1;
				*total_remaining -=1;
				if *step_remaining == 0
				{
					//When the whole step is performed advance `next_provenance`.
					let (ref _pattern, _total_messages, _message_size, step_size) = self.provenance[provenance_index];
					*step_remaining = step_size;
					self.next_provenance[origin] = (provenance_index+1) % pending.len();
				}
				break;
			}
			provenance_index = (provenance_index+1) % pending.len();
		}
		// Build the message
		let (ref pattern,_total_messages,message_size,_step_size) = self.provenance[provenance_index];
		let destination=pattern.get_destination(origin,topology,rng);
		if origin==destination
		{
			return Err(TrafficError::SelfMessage);
		}
		let id = self.next_id;
		self.next_id += 1;
		let message = Rc::new(Message{
			origin,
			destination,
			size:message_size,
			creation_cycle: cycle,
			payload: id.to_le_bytes().into(),
            id_traffic: None,
        });
		self.generated_messages.insert(id);
		Ok(message)
	}
	fn probability_per_cycle(&self, task:usize) -> f32
	{
		for (total_remaining,_step_remaining) in self.pending[task].iter()
		{
			if *total_remaining > 0
			{
				return 1.0;
			}
		}
		0.0
	}
	fn consume(&mut self, _task:usize, message: &dyn AsMessage, _cycle:Time, _topology:&dyn Topology, _rng: &mut StdRng) -> bool
	{
		//let message_ptr=message.as_ref() as *const Message;
		//self.generated_messages.remove(&message_ptr)
		let id = u128::from_le_bytes(message.payload()[0..16].try_into().expect("bad payload"));
		self.generated_messages.remove(&id)
	}
	fn is_finished(&self) -> bool
	{
		if !self.generated_messages.is_empty()
		{
			return false;
		}
		for task_pending in self.pending.iter()
		{
			for (total_remaining, _step_remaining) in task_pending.iter()
			{
				if *total_remaining > 0
				{
					return false;
				}
			}
		}
		true
	}
	fn task_state(&self, task:usize, _cycle:Time) -> Option<TaskTrafficState>
	{
		if self.pending[task].iter().any(|(total_remaining,_step_remaining)| *total_remaining > 0 ) {
			Some(TaskTrafficState::Generating)
		} else {
			//We do not know whether someone is sending us data.
			//if self.is_finished() { TaskTrafficState::Finished } else { TaskTrafficState::UnspecifiedWait }
			// Sometimes it could be Finished, but it is not worth computing...
			Some(TaskTrafficState::FinishedGenerating)
		}
	}

	fn number_tasks(&self) -> usize {
		self.tasks
	}
}

impl MultimodalBurst
{
	pub fn new(arg:TrafficBuilderArgument) -> MultimodalBurst
	{
		let mut tasks=None;
		let mut provenance : Option<Vec<(_,_,_,_)>> = None;
		match_object_panic!(arg.cv,"MultimodalBurst",value,
			"tasks" | "servers" => tasks=Some(value.as_f64().expect("bad value for tasks") as usize),
			"provenance" => match value
			{
				&ConfigurationValue::Array(ref a) => provenance=Some(a.iter().map(|pcv|{
					let mut messages_per_task=None;
					let mut pattern=None;
					let mut message_size=None;
					let mut step_size=None;
					match_object_panic!(pcv,"Provenance",pvalue,
						"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:pvalue,plugs:arg.plugs})),
						"messages_per_task" | "messages_per_server" | "total_messages" =>
							messages_per_task=Some(pvalue.as_f64().expect("bad value for messages_per_task") as usize),
						"message_size" => message_size=Some(pvalue.as_f64().expect("bad value for message_size") as usize),
						"step_size" => step_size=Some(pvalue.as_f64().expect("bad value for step_size") as usize),
					);
					let pattern=pattern.expect("There were no pattern");
					let messages_per_task=messages_per_task.expect("There were no messages_per_task");
					let message_size=message_size.expect("There were no message_size");
					let step_size=step_size.expect("There were no step_size");
					(pattern,messages_per_task,message_size,step_size)
				}).collect()),
				_ => panic!("bad value for provenance"),
			}
		);
		let tasks=tasks.expect("There were no tasks");
		let mut provenance=provenance.expect("There were no provenance");
		for (pattern,_total_messages,_message_size,_step_size) in provenance.iter_mut()
		{
			pattern.initialize(tasks, tasks, arg.topology, arg.rng);
		}
		let each_pending = provenance.iter().map(|(_pattern,total_messages,_message_size,step_size)|(*total_messages,*step_size)).collect();
		MultimodalBurst{
			tasks,
			provenance,
			pending: vec![each_pending;tasks],
			next_provenance:vec![0;tasks],
			generated_messages: BTreeSet::new(),
			next_id: 0,
		}
	}
}




/**
Selects the traffic from a sequence depending on current cycle. This traffics is useful to make sequences of traffics that do no end by themselves.

All the subtraffics in `traffics` must give the same value for `number_tasks`, which is also used for TimeSequenced. At least one such subtraffic must be provided.

```ignore
TimeSequenced{
	traffics: [HomogeneousTraffic{...}, HomogeneousTraffic{...}],
	times: [2000, 15000],
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct TimeSequenced
{
    ///List of applicable traffics.
    traffics: Vec<Box<dyn Traffic>>,
    ///End time of each traffic. Counting from the end of the previous one.
    times: Vec<Time>,
}

impl Traffic for TimeSequenced
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        let mut offset = cycle;
        let mut traffic_index = 0;
        while traffic_index<self.traffics.len() && offset >= self.times[traffic_index]
        {
            offset -= self.times[traffic_index];
            traffic_index += 1;
        }
        assert!(traffic_index<self.traffics.len());
        self.traffics[traffic_index].generate_message(origin,cycle,topology,rng)
    }
    fn probability_per_cycle(&self,_task:usize) -> f32
    {
        //Can we do better here?
        1.0
    }
    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        for traffic in self.traffics.iter_mut()
        {
            if traffic.consume(task, message, cycle, topology, rng)
            {
                return true;
            }
        }
        return false;
    }
    fn is_finished(&self) -> bool
    {
        //This is a bit silly for a time sequence
        for traffic in self.traffics.iter()
        {
            if !traffic.is_finished()
            {
                return false;
            }
        }
        return true;
    }
    fn should_generate(&mut self, task:usize, cycle:Time, rng: &mut StdRng) -> bool
    {
        let mut offset = cycle;
        let mut traffic_index = 0;
        while traffic_index<self.traffics.len() && offset >= self.times[traffic_index]
        {
            offset -= self.times[traffic_index];
            traffic_index += 1;
        }
        if traffic_index<self.traffics.len(){
            self.traffics[traffic_index].should_generate(task,cycle,rng)
        } else {
            false
        }
    }
    fn task_state(&self, task:usize, cycle:Time) -> Option<TaskTrafficState>
    {
        let mut offset = cycle;
        let mut traffic_index = 0;
        while traffic_index<self.traffics.len() && offset >= self.times[traffic_index]
        {
            offset -= self.times[traffic_index];
            traffic_index += 1;
        }
        if traffic_index == self.traffics.len()
        {
            return Some(Finished);
        }
        let state = self.traffics[traffic_index].task_state(task,cycle).expect("TODO! the none case");
        if let Finished = state {
            Some(WaitingCycle { cycle:self.times[traffic_index] })
        } else {
            Some(state)
        }
    }

    fn number_tasks(&self) -> usize {
        // each traffic has the same number of tasks
        self.traffics[0].number_tasks()
    }
}

impl TimeSequenced
{
    pub fn new(mut arg:TrafficBuilderArgument) -> TimeSequenced
    {
        let mut traffics : Option<Vec<_>> =None;
        let mut times=None;
        match_object_panic!(arg.cv,"TimeSequenced",value,
			"traffics" => traffics = Some(value.as_array().expect("bad value for traffics").iter()
				.map(|v|new_traffic(TrafficBuilderArgument{cv:v,rng:&mut arg.rng,..arg})).collect()),
			"times" => times = Some(value.as_array()
				.expect("bad value for times").iter()
				.map(|v|v.as_time().expect("bad value in times")).collect()),
		);
        let traffics=traffics.expect("There were no traffics");
        assert!( !traffics.is_empty() , "Cannot make a TimeSequenced of 0 traffics." );
        let size = traffics[0].number_tasks();
        for traffic in traffics.iter().skip(1)
        {
            assert_eq!( traffic.number_tasks(), size , "In TimeSequenced all sub-traffics must involve the same number of tasks." );
        }
        let times=times.expect("There were no times");
        TimeSequenced{
            traffics,
            times,
        }
    }
}

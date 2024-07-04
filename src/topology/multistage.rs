
use std::collections::BTreeSet;

use ::rand::{Rng,rngs::StdRng};
use quantifiable_derive::Quantifiable;//the derive macro
use std::fs::File;
use std::io::{BufRead,BufReader};
use std::cell::RefCell;
use crate::Time;

use super::{Topology,Location,TopologyBuilderArgument,
	cartesian::CartesianData,
	projective::FlatGeometryCache,
	NeighbourRouterIteratorItem};

use crate::{
	Plugs,error,source_location,
	config_parser::ConfigurationValue,matrix::Matrix,
	error::{Error,SourceLocation},
	};
use crate::quantify::Quantifiable;

use crate::routing::{RoutingInfo,Routing,CandidateEgress,RoutingBuilderArgument,RoutingNextCandidates};

///Requirements on each level. They are combined by the multiple stages of a MultiStage topology aiming to get values compatible with all of them.
#[derive(Debug,Clone,Copy)]
pub struct LevelRequirements
{
	pub group_size: usize,
	pub current_level_minimum_size: usize,
}

impl Default for LevelRequirements
{
	fn default() -> Self
	{
		LevelRequirements{ group_size:1, current_level_minimum_size:1 }
	}
}

//TODO: fix nomenclature. TO be coherent with random stages ignoring any kind of actual multiplier or grouping.
pub trait Stage : Quantifiable + std::fmt::Debug
{
	// ///The subnetwork below this stage is replicated `below_multiplier()` times.
	// ///The stage 'sees' the top-level routers of such subnetwork with the same multiplier.
	// fn below_multiplier(&self) -> usize;
	// ///The number of top routers of the stage will be the the number of top routers in the subnetwork multiplied by `above_multiplier()`.
	// fn above_multiplier(&self) -> usize;
	// ///Verifies whether the Stage can be defined with the given amount of routers.
	// ///At least it is required `below_size*above_multiplier()==above_size*below_multiplier()`, but the stage may impose additinal constraints.
	// ///For example, a map read from a file will only work with some specific values; and a randomly generated may work with any.
	// fn verify(&self,below_size:usize,above_size:usize) -> bool;
	///Compose the requirements of lower stages with this one.
	///`bottom_level` is the level corresponding to the bottom of this Stage.
	///`height` is the total number of stages in the network.
	fn compose_requirements_upward(&self,requirements:LevelRequirements,bottom_level:usize,height:usize) -> LevelRequirements;
	///Compute the size of the bottom level given the top one.
	///Return error if there is not a legal one.
	fn downward_size(&self,top_size:usize,bottom_group_size:usize,bottom_level:usize,height:usize) -> Result<usize,Error>;
	///Number of top routers that are neighbour to a given bottom.
	fn amount_to_above(&self,below_router:usize,group_size:usize, bottom_size:usize) -> usize;
	///Number of bottom routers that are neighbour to a given top.
	fn amount_to_below(&self,above_router:usize,group_size:usize, bottom_size:usize) -> usize;
	///Get the (top neighbour, reverse index) of a bottom router associated to the given `index`. `0<=index<amount_to_above()`.
	///`group_size` is the size of each subnetwork.
	fn to_above(&self, below_router:usize, index:usize, group_size:usize, bottom_size:usize) -> (usize,usize);
	///Get the (bottom neighbour, reverse index) of a top router associated to the given `index`. `0<=index<amount_to_below()`.
	///`group_size` is the size of each subnetwork.
	fn to_below(&self, above_router:usize, index:usize, group_size:usize, bottom_size:usize) -> (usize,usize);
}


///Each stage of a XGFT.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct FatStage
{
	bottom_factor: usize,
	top_factor: usize,
}

impl Stage for FatStage
{
	//fn below_multiplier(&self) -> usize
	//{
	//	self.bottom_factor
	//}
	//fn above_multiplier(&self) -> usize
	//{
	//	self.top_factor
	//}
	//fn verify(&self,below_size:usize,above_size:usize) -> bool
	//{
	//	below_size*self.top_factor == above_size*self.bottom_factor
	//}
	fn compose_requirements_upward(&self,requirements:LevelRequirements,_bottom_level:usize,_height:usize) -> LevelRequirements
	{
		LevelRequirements{
			group_size: requirements.group_size*self.top_factor,
			current_level_minimum_size: requirements.current_level_minimum_size*self.top_factor,
		}
	}
	fn downward_size(&self,top_size:usize,_bottom_group_size:usize,_bottom_level:usize,_heigh:usize) -> Result<usize,Error>
	{
		let partial = top_size * self.bottom_factor;
		if partial % self.top_factor == 0
		{
			Ok(partial/self.top_factor)
		}
		else
		{
			Err(error!(undetermined))
		}
	}
	fn amount_to_above(&self,_below_router:usize,_group_size:usize, _bottom_size:usize) -> usize
	{
		self.top_factor
	}
	fn amount_to_below(&self,_above_router:usize,_group_size:usize, _bottom_size:usize) -> usize
	{
		self.bottom_factor
	}
	fn to_above(&self, below_router:usize, index:usize, group_size:usize, _bottom_size:usize) -> (usize,usize)
	{
		let above_group_size = group_size * self.top_factor;
		let below_group_size = group_size * self.bottom_factor;
		let group=below_router/below_group_size;
		let offset=below_router%below_group_size;
		let quotient = offset / group_size;
		let remainder = offset % group_size;
		(remainder+index*group_size+group*above_group_size,quotient)
	}
	fn to_below(&self, above_router:usize, index:usize, group_size:usize, _bottom_size:usize) -> (usize,usize)
	{
		let above_group_size = group_size * self.top_factor;
		let below_group_size = group_size * self.bottom_factor;
		let group=above_router/above_group_size;
		let offset=above_router%above_group_size;
		let quotient = offset / group_size;
		let remainder = offset % group_size;
		(remainder+index*group_size+group*below_group_size,quotient)
	}
}

impl FatStage
{
	pub fn new(arg:StageBuilderArgument) -> FatStage
	{
		let mut bottom_factor=None;
		let mut top_factor=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Fat"
			{
				panic!("A Fat must be created from a `Fat` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"bottom_factor" => match value
					{
						&ConfigurationValue::Number(f) => bottom_factor=Some(f as usize),
						_ => panic!("bad value for bottom_factor"),
					},
					"top_factor" => match value
					{
						&ConfigurationValue::Number(f) => top_factor=Some(f as usize),
						_ => panic!("bad value for top_factor"),
					},
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in Fat",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a Fat from a non-Object");
		}
		let bottom_factor=bottom_factor.expect("There were no bottom_factor");
		let top_factor=top_factor.expect("There were no top_factor");
		FatStage{
			bottom_factor,
			top_factor,
		}
	}
}


///The stages in a Orthogonal Fat Tree
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ProjectiveStage
{
	plane: FlatGeometryCache,
}

impl Stage for ProjectiveStage
{
	//fn below_multiplier(&self) -> usize
	//{
	//	self.plane.geometry.amount_points()
	//}
	//fn above_multiplier(&self) -> usize
	//{
	//	self.plane.geometry.amount_lines()
	//}
	//fn verify(&self,below_size:usize,above_size:usize) -> bool
	//{
	//	below_size*self.above_multiplier() == above_size*self.below_multiplier()
	//}
	fn compose_requirements_upward(&self,requirements:LevelRequirements,_bottom_level:usize,_height:usize) -> LevelRequirements
	{
		let top_factor = self.plane.geometry.amount_lines();
		//if bottom_level+1==height
		//{
		//	//Last level has half the routers.
		//	top_factor /= 2;
		//}
		LevelRequirements{
			group_size: requirements.group_size*top_factor,
			current_level_minimum_size: requirements.current_level_minimum_size*top_factor,
		}
	}
	fn downward_size(&self,top_size:usize,_bottom_group_size:usize,_bottom_level:usize,_height:usize) -> Result<usize,Error>
	{
		let partial = top_size * self.plane.geometry.amount_points();
		let top_factor = self.plane.geometry.amount_lines();
		//if bottom_level+1==height
		//{
		//	//Last level has half the routers.
		//	top_factor /= 2;
		//}
		if partial % top_factor == 0
		{
			Ok(partial/top_factor)
		}
		else
		{
			Err(error!(undetermined))
		}
	}
	fn amount_to_above(&self,below_router:usize, group_size: usize, _bottom_size:usize) -> usize
	{
		//let below_group_size = group_size * self.below_multiplier();
		let below_group_size = group_size * self.plane.geometry.amount_points();
		let offset=below_router%below_group_size;
		let quotient = offset / group_size;
		self.plane.lines_by_point[quotient].len()
	}
	fn amount_to_below(&self,above_router:usize, group_size: usize, _bottom_size:usize) -> usize
	{
		//let above_group_size = group_size * self.above_multiplier();
		let above_group_size = group_size * self.plane.geometry.amount_lines();
		let offset=above_router%above_group_size;
		let quotient = offset / group_size;
		self.plane.points_by_line[quotient].len()
	}
	fn to_above(&self, below_router:usize, index:usize, group_size:usize, _bottom_size:usize) -> (usize,usize)
	{
		//let above_group_size = group_size * self.above_multiplier();
		let above_group_size = group_size * self.plane.geometry.amount_lines();
		//let below_group_size = group_size * self.below_multiplier();
		let below_group_size = group_size * self.plane.geometry.amount_points();
		let group=below_router/below_group_size;
		let offset=below_router%below_group_size;
		let quotient = offset / group_size;
		let remainder = offset % group_size;
		let (line,line_index) = self.plane.lines_by_point[quotient][index];
		(remainder+line*group_size+group*above_group_size,line_index)
	}
	fn to_below(&self, above_router:usize, index:usize, group_size:usize, _bottom_size:usize) -> (usize,usize)
	{
		//let above_group_size = group_size * self.above_multiplier();
		let above_group_size = group_size * self.plane.geometry.amount_lines();
		//let below_group_size = group_size * self.below_multiplier();
		let below_group_size = group_size * self.plane.geometry.amount_points();
		let group=above_router/above_group_size;
		let offset=above_router%above_group_size;
		let quotient = offset / group_size;
		let remainder = offset % group_size;
		let (point,point_index) = self.plane.points_by_line[quotient][index];
		(remainder+point*group_size+group*below_group_size,point_index)
	}
}

impl ProjectiveStage
{
	pub fn new(arg:StageBuilderArgument) -> ProjectiveStage
	{
		let mut prime=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Projective"
			{
				panic!("A Projective must be created from a `Projective` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"prime" => match value
					{
						&ConfigurationValue::Number(f) => prime=Some(f as usize),
						_ => panic!("bad value for prime"),
					},
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in Projective",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a Projective from a non-Object");
		}
		let prime=prime.expect("There were no prime");
		ProjectiveStage{
			plane: FlatGeometryCache::new_prime(prime).unwrap_or_else(|_|panic!("{} is not prime, which is required for the ProjectiveStage",prime)),
		}
	}
}


/**
MLFM stage from Fujitsu.
 More details in: https://www.fujitsu.com/global/about/resources/news/press-releases/2014/0715-02.html
FMStage{
	layers: 4, // number of layers.
	layer_size: 8, // number of routers per layer.
}
**/
#[derive(Quantifiable)]
#[derive(Debug)]
struct FMStage
{
	layers: usize,
	layer_size: usize,
}

impl Stage for FMStage
{
	fn compose_requirements_upward(&self,_requirements:LevelRequirements,_bottom_level:usize,_height:usize) -> LevelRequirements
	{
		LevelRequirements{
			group_size: self.layer_size * (self.layer_size -1) /2,
			current_level_minimum_size: self.layer_size * (self.layer_size -1) /2,
		}
	}
	fn downward_size(&self,top_size:usize,_bottom_group_size:usize,_bottom_level:usize,_height:usize) -> Result<usize,Error>
	{
		let mut layer_size = 0;

		while layer_size *(layer_size -1)/2 < top_size {
			layer_size +=1;
		}

		if layer_size *(layer_size -1)/2 == top_size
		{
			Ok(layer_size * self.layers)

		}else{
			Err(error!(undetermined))
		}

	}
	fn amount_to_above(&self,_below_router:usize, _group_size: usize, _bottom_size:usize) -> usize
	{
		self.layer_size -1
	}
	fn amount_to_below(&self,_above_router:usize, _group_size: usize, _bottom_size:usize) -> usize
	{
		2*self.layers
	}
	fn to_above(&self, below_router:usize, index:usize, _group_size:usize, _bottom_size:usize) -> (usize,usize)
	{
		let layer = below_router / self.layers;
		if index >= layer
		{
			(layer * self.layer_size - ((layer +1) * layer)/2 + (index - layer), below_router % self.layers)

		}else{
			let layer_2 = layer -1;

			(index * self.layer_size - ((index+1) * (index))/2 + (layer_2 - index), (below_router % self.layers) + self.layers)
		}
	}
	fn to_below(&self, above_router:usize, index:usize, _group_size:usize, _bottom_size:usize) -> (usize,usize)
	{

		let mut a_index = 0;
		let mut count = 0;

		while (count + self.layer_size - (a_index + 1) )-1 < above_router {
			a_index += 1;
			count += self.layer_size - a_index;

			//print!("index {}", a_index)
		}
		//a_index -= 1;

		if index < self.layers
		{
			(a_index * self.layers + index, a_index + above_router - count)	//
		}
		else{

			//print!("to_below")

			((a_index +  above_router - count+1) * self.layers + index%self.layers, a_index )
		}
	}
}


impl FMStage
{
	pub fn new(arg:StageBuilderArgument) -> FMStage
	{
		let mut layers =None;
		let mut layer_size =None;

		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="FMStage"
			{
				panic!("A FMStage must be created from a `FMStage` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"layers" => match value
					{
						&ConfigurationValue::Number(f) => layers =Some(f as usize),
						_ => panic!("bad value for k"),
					},
					"layer_size" => match value
					{
						&ConfigurationValue::Number(f) => layer_size =Some(f as usize),
						_ => panic!("bad value for pods"),
					},
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in FMStage",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a FMStage from a non-Object");
		}
		let layers= layers.expect("There were no k");
		let layer_size= layer_size.expect("There were no pods");
		FMStage{
			layers,
			layer_size,
			//lane: FlatGeometryCache::new_prime(prime).unwrap_or_else(|_|panic!("{} is not prime, which is required for the ProjectiveStage",prime)),
		}
	}
}

///A Stage with a explicitly given list of neighbours for each router. Ignores grouping.
///Apt to build random stages.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ExplicitStage
{
	///Number of routers in the bottom level.
	bottom_size: usize,
	///Number of routers in the top level.
	top_size: usize,
	bottom_list: Vec<Vec<(usize,usize)>>,
	top_list: Vec<Vec<(usize,usize)>>,
}

impl Stage for ExplicitStage
{
	//fn below_multiplier(&self) -> usize
	//{
	//	todo!()
	//}
	//fn above_multiplier(&self) -> usize
	//{
	//	todo!()
	//}
	//fn verify(&self,below_size:usize,above_size:usize) -> bool
	//{
	//	below_size==self.bottom_size && above_size==self.top_size
	//}
	fn compose_requirements_upward(&self,requirements:LevelRequirements,_bottom_level:usize,_height:usize) -> LevelRequirements
	{
		if self.bottom_size % requirements.current_level_minimum_size != 0
		{
			panic!("This size cannot be satisfied by the ExplicitStage");
		}
		LevelRequirements{
			group_size: 1,
			current_level_minimum_size: self.top_size,
		}
	}
	fn downward_size(&self,top_size:usize,_bottom_group_size:usize,_bottom_level:usize,_height:usize) -> Result<usize,Error>
	{
		if top_size==self.top_size
		{
			Ok(self.bottom_size)
		}
		else
		{
			Err(error!(undetermined))
		}
	}
	fn amount_to_above(&self,below_router:usize,_group_size:usize, _bottom_size:usize) -> usize
	{
		self.bottom_list[below_router].len()
	}
	fn amount_to_below(&self,above_router:usize,_group_size:usize, _bottom_size:usize) -> usize
	{
		self.top_list[above_router].len()
	}
	fn to_above(&self, below_router:usize, index:usize, _group_size:usize, _bottom_size:usize) -> (usize,usize)
	{
		self.bottom_list[below_router][index]
	}
	fn to_below(&self, above_router:usize, index:usize, _group_size:usize, _bottom_size:usize) -> (usize,usize)
	{
		self.top_list[above_router][index]
	}
}

impl ExplicitStage
{
	pub fn new(arg:StageBuilderArgument) -> ExplicitStage
	{
		let mut bottom_size=None;
		let mut top_size=None;
		let mut upwards_degree=None;
		let mut downwards_degree=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="RandomRegular"
			{
				panic!("A RandomRegular must be created from a `RandomRegular` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"bottom_size" => match value
					{
						&ConfigurationValue::Number(f) => bottom_size=Some(f as usize),
						_ => panic!("bad value for bottom_size"),
					},
					"top_size" => match value
					{
						&ConfigurationValue::Number(f) => top_size=Some(f as usize),
						_ => panic!("bad value for top_size"),
					},
					"upwards_degree" => match value
					{
						&ConfigurationValue::Number(f) => upwards_degree=Some(f as usize),
						_ => panic!("bad value for upwards_degree"),
					},
					"downwards_degree" => match value
					{
						&ConfigurationValue::Number(f) => downwards_degree=Some(f as usize),
						_ => panic!("bad value for downwards_degree"),
					},
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in RandomRegular",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a RandomRegular from a non-Object");
		}
		let bottom_size=bottom_size.expect("There were no bottom_size");
		let top_size=top_size.expect("There were no top_size");
		let upwards_degree=upwards_degree.expect("There were no upwards_degree");
		let downwards_degree=downwards_degree.expect("There were no downwards_degree");
		let (upwards,downwards) = ExplicitStage::random_adjacencies(bottom_size,upwards_degree,top_size,downwards_degree,arg.rng);
		let (bottom_list,top_list) = ExplicitStage::add_reverse_indices(&upwards,&downwards);
		ExplicitStage{
			bottom_size,
			top_size,
			bottom_list,
			top_list,
		}
	}
	///Convert a pair of list of adjacencies into a pair of lists including the index to return.
	///This is, return (f,g) with `g[f[x][i].0][f[x][i].1]=x` and `f[g[x][i].0][g[x][i].1]=x` for any `x` in range.
	pub fn add_reverse_indices(to_above:&[Vec<usize>],to_below:&[Vec<usize>]) -> (Vec<Vec<(usize,usize)>>,Vec<Vec<(usize,usize)>>)
	{
		let bottom_list=to_above.iter().enumerate().map(|(current,neighbours)|
			neighbours.iter().map(|&neigh|(neigh,
			{
				let mut index=0;
				for (i,&v) in to_below[neigh].iter().enumerate()
				{
					if v==current
					{
						index=i;
						break;
					}
				}
				index
			})).collect()
		).collect();
		let top_list=to_below.iter().enumerate().map(|(current,neighbours)|
			neighbours.iter().map(|&neigh|(neigh,
			{
				let mut index=0;
				for (i,&v) in to_above[neigh].iter().enumerate()
				{
					if v==current
					{
						index=i;
						break;
					}
				}
				index
			})).collect()
		).collect();
		(bottom_list,top_list)
	}
	///Build random regular adjacencies.
	pub fn random_adjacencies(bottom_size:usize, bottom_degree:usize, top_size:usize, top_degree:usize, rng: &mut StdRng) -> (Vec<Vec<usize>>,Vec<Vec<usize>>)
	{
		let mut to_above=vec![Vec::with_capacity(bottom_degree);bottom_size];
		let mut to_below=vec![Vec::with_capacity(top_degree);top_size];
		let mut go=true;
		while go
		{
			go=false;
			let mut upwards_available_amount=bottom_size*bottom_degree;
			let mut upwards_available=(0..bottom_size*bottom_degree).collect::<Vec<usize>>();
			let mut downwards_available_amount=top_size*top_degree;
			let mut downwards_available=(0..top_size*top_degree).collect::<Vec<usize>>();
			for adjs in to_above.iter_mut()
			{
				adjs.clear();
			}
			for adjs in to_below.iter_mut()
			{
				adjs.clear();
			}
			let mut upwards_remaining=(0..bottom_size).collect::<BTreeSet<usize>>();
			let mut downwards_remaining=(0..top_size).collect::<BTreeSet<usize>>();
			while upwards_available_amount>0
			{
				//Check that there is some new link among the remainder routers.
				if upwards_remaining.len()<bottom_degree && downwards_remaining.len()<top_degree
				{
					//This could be improved into counting the number of available links and comparing it with the number of required ones.
					let mut good=false;
					for &i in upwards_remaining.iter()
					{
						for &j in downwards_remaining.iter()
						{
							let mut inadj=false;
							for &neigh in to_above[i].iter()
							{
								if neigh==j
								{
									inadj=true;
									break;
								}
							}
							if !inadj
							{
								good=true;
							}
						}
					}
					if !good
					{
						go=true;
						break;
					}
				}
				//sample points x,y, keep them last in U to remove them in O(1)
				//let r=rng.gen_range(0,upwards_available_amount);//rand-0.4
				let r=rng.gen_range(0..upwards_available_amount);//rand-0.8
				let x=upwards_available[r];
				upwards_available[r]=upwards_available[upwards_available_amount-1];
				upwards_available[upwards_available_amount-1]=x;

				let r=rng.gen_range(0..downwards_available_amount);
				let y=downwards_available[r];
				downwards_available[r]=downwards_available[downwards_available_amount-1];
				downwards_available[downwards_available_amount-1]=y;

				//vertex_index u=x/degree, v=y/degree;//vertices
				let u=x/bottom_degree;
				let v=y/top_degree;
				
				let mut inadj=false;
				for &neigh in to_above[u].iter()
				{
					if neigh==v
					{
						inadj=true;
						break;
					}
				}
				if inadj
				{
					continue;//no multiple edges
				}
				upwards_available_amount-=1;
				downwards_available_amount-=1;
				to_above[u].push(v);
				if to_above[u].len()==bottom_degree
				{
					upwards_remaining.remove(&u);
				}
				to_below[v].push(u);
				if to_below[v].len()==top_degree
				{
					downwards_remaining.remove(&v);
				}
			}
		}
		(to_above,to_below)
	}
}


/**
A Stage with a explicitly given list of neighbours for each router. Ignores grouping.
Apt to build random stages.

TODO: document and example.
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ExplicitStageFile
{
	///Number of routers in the bottom level.
	bottom_size: usize,
	///Number of routers in the top level.
	top_size: usize,
	bottom_list: Vec<Vec<(usize,usize)>>,
	top_list: Vec<Vec<(usize,usize)>>,
}

impl Stage for ExplicitStageFile
{
	//fn below_multiplier(&self) -> usize
	//{
	//	todo!()
	//}
	//fn above_multiplier(&self) -> usize
	//{
	//	todo!()
	//}
	//fn verify(&self,below_size:usize,above_size:usize) -> bool
	//{
	//	below_size==self.bottom_size && above_size==self.top_size
	//}

	fn compose_requirements_upward(&self,requirements:LevelRequirements,_bottom_level:usize,_height:usize) -> LevelRequirements
	{
		if self.bottom_size % requirements.current_level_minimum_size != 0
		{
			panic!("This size cannot be satisfied by the ExplicitStage");
		}
		LevelRequirements{
			group_size: 1,
			current_level_minimum_size: self.top_size,
		}
	}
	fn downward_size(&self,top_size:usize,_bottom_group_size:usize,_bottom_level:usize,_height:usize) -> Result<usize,Error>
	{
		if top_size==self.top_size
		{
			Ok(self.bottom_size)
		}
		else
		{
			Err(error!(undetermined))
		}
	}
	fn amount_to_above(&self,below_router:usize,_group_size:usize, _bottom_size:usize) -> usize
	{
		self.bottom_list[below_router].len()
	}
	fn amount_to_below(&self,above_router:usize,_group_size:usize, _bottom_size:usize) -> usize
	{
		self.top_list[above_router].len()
	}
	fn to_above(&self, below_router:usize, index:usize, _group_size:usize, _bottom_size:usize) -> (usize,usize)
	{
		self.bottom_list[below_router][index]
	}
	fn to_below(&self, above_router:usize, index:usize, _group_size:usize, _bottom_size:usize) -> (usize,usize)
	{
		self.top_list[above_router][index]
	}

}

pub fn file_adj(file:&File, _format:usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>, usize, usize)
	{
		//let mut adj=vec![Vec::with_capacity(degree);routers];
		let mut adj_down : Vec<Vec<usize>> =vec![];
		let mut adj_up : Vec<Vec<usize>> =vec![];
		let mut nodos_up=None;
		let mut nodos_down=None;
		let reader = BufReader::new(file);
		let mut lines=reader.lines();
		//for rline in reader.lines()
		while let Some(rline)=lines.next()
		{
			let line=rline.expect("Some problem when reading the topology.");
			//println!("line: {}",line);
			let mut words=line.split_whitespace();
			match words.next()
			{
				Some("NODOS_0") =>
				{
					nodos_down=Some(words.next().unwrap().parse::<usize>().unwrap())
					//assert!( nodos % 2 == 0); //should be even
				},
				Some("GRADO_0") =>
				{
					let grado=Some(words.next().unwrap().parse::<usize>().unwrap());
					if let Some(routers)=nodos_down
					{
						assert!( routers % 2 == 0); //should be even
						if let Some(degree)=grado
						{
							adj_up=vec![Vec::with_capacity(degree);routers]; //just half
						}
					}
				},

				Some("NODOS_1") =>
				{
					nodos_up=Some(words.next().unwrap().parse::<usize>().unwrap());
					//assert!( nodos % 2 == 0); //should be even
				},
				Some("GRADO_1") =>
				{
					let grado=Some(words.next().unwrap().parse::<usize>().unwrap());
					if let Some(routers)=nodos_up
					{
						//assert!( routers % 2 == 0); //should be even
						if let Some(degree)=grado
						{
							adj_down=vec![Vec::with_capacity(degree);routers];
						}
					}
				},
				Some("N") =>
				{

					let nodos_down = nodos_down.expect("There should be a number, if not bad format");
					//let nodos_up = nodos_up.expect("There should be a number, if not bad format");
					let current=words.next().unwrap().parse::<usize>().unwrap();

					for wneighbour in lines.next().unwrap().unwrap().split_whitespace()
					{
						let mut neighbour = wneighbour.parse::<usize>().unwrap();

						if neighbour >= nodos_down{
							neighbour -= nodos_down; //just to normalize, it should be done in an more efficient way, to avoid bugs also...
						}

						if current < nodos_down{

							adj_up[current].push(neighbour);

						}else{
							let new_index = current - nodos_down;
							adj_down[new_index].push(neighbour);

						}
					}
				},
				_ => panic!("Illegal word"),
			};
		}

		(adj_up ,adj_down, nodos_down.expect("There should be a number, if not bad format"), nodos_up.expect("There should be a number, if not bad format"))
	}



impl ExplicitStageFile
{
	pub fn new(arg:StageBuilderArgument) -> ExplicitStageFile
	{
		let mut filename=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="ExplicitStageFile"
			{
				panic!("A ExplicitStageFile must be created from a `ExplicitStageFile` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"filename" => match value
					{
						&ConfigurationValue::Literal(ref s) => filename=Some(s.to_string()),
						_ => panic!("bad value for filename"),
					},
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in RandomRegular",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a RandomRegular from a non-Object");
		}
		let format = 0;
		let filename=filename.expect("There were no filename");
		let file=File::open(&filename).expect("could not open topology file.");
		let (upwards,downwards,size_down, size_up) = file_adj(&file,format);
		let (bottom_list,top_list) = ExplicitStage::add_reverse_indices(&upwards,&downwards);

		ExplicitStageFile{
			bottom_size: size_down,
			top_size: size_up,
			bottom_list,
			top_list,
		}
	}
}



#[derive(Quantifiable)]
#[derive(Debug)]
pub struct WidenedStage
{
	base: Box<dyn Stage>,
	multiplier: usize,
}

impl Stage for WidenedStage
{
	fn compose_requirements_upward(&self,requirements:LevelRequirements,bottom_level:usize,height:usize) -> LevelRequirements
	{
		self.base.compose_requirements_upward(requirements,bottom_level,height)
	}
	fn downward_size(&self,top_size:usize,bottom_group_size:usize,bottom_level:usize,height:usize) -> Result<usize,Error>
	{
		let base_downward_size = self.base.downward_size(top_size,bottom_group_size,bottom_level,height)?;
		Ok(base_downward_size * self.multiplier)
	}
	fn amount_to_above(&self,below_router:usize, group_size: usize, bottom_size: usize) -> usize
	{
		let base_bottom_size = bottom_size/self.multiplier;
		let base_below_router = below_router % base_bottom_size;
		self.base.amount_to_above(base_below_router, group_size, base_bottom_size)
	}
	fn amount_to_below(&self,above_router:usize, group_size: usize, bottom_size: usize) -> usize
	{
		let base_bottom_size = bottom_size/self.multiplier;
		let base_deg = self.base.amount_to_below(above_router,group_size,base_bottom_size);
		base_deg * self.multiplier
	}
	fn to_above(&self, below_router:usize, index:usize, group_size:usize, bottom_size: usize) -> (usize,usize)
	{
		let base_bottom_size = bottom_size/self.multiplier;
		let base_below_router = below_router % base_bottom_size;
		let quotient = below_router / base_bottom_size;
		let (neighbour,rev_index) = self.base.to_above(base_below_router, index, group_size, base_bottom_size);
		let base_deg = self.base.amount_to_below(neighbour,group_size,base_bottom_size);
		(neighbour,rev_index + quotient*base_deg)
	}
	fn to_below(&self, above_router:usize, index:usize, group_size:usize, bottom_size: usize) -> (usize,usize)
	{
		let base_bottom_size = bottom_size/self.multiplier;
		let base_deg = self.base.amount_to_below(above_router,group_size,base_bottom_size);
		let quotient = index / base_deg;
		let remainder = index % base_deg;
		let (neighbour,rev_index) = self.base.to_below(above_router, remainder, group_size, base_bottom_size);
		(neighbour + quotient*base_bottom_size, rev_index)
	}
}

impl WidenedStage
{
	pub fn new(mut arg:StageBuilderArgument) -> WidenedStage
	{
		let mut base=None;
		let mut multiplier=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Widened"
			{
				panic!("A Widened must be created from a `Widened` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"base" => base=Some(new_stage(StageBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
					"multiplier" => match value
					{
						&ConfigurationValue::Number(f) => multiplier=Some(f as usize),
						_ => panic!("bad value for multiplier"),
					},
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in Widened",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a Widened from a non-Object");
		}
		let base=base.expect("There were no base");
		let multiplier=multiplier.expect("There were no multiplier");
		WidenedStage{
			base,
			multiplier,
		}
	}
}


/**
OFT fault tolerance stage, with paralell links.

TODO: document and example.
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct FaultToleranceStage
{
	base: Box<dyn Stage>,
	redundancy: usize,
}

impl Stage for FaultToleranceStage
{
	fn compose_requirements_upward(&self,requirements:LevelRequirements,bottom_level:usize,height:usize) -> LevelRequirements
	{

		let mut req = self.base.compose_requirements_upward(requirements,bottom_level,height);
		//println!("requirements {},{} bottom_level {} height {}",req.current_level_minimum_size,req.group_size,bottom_level, height);
		req.current_level_minimum_size = req.current_level_minimum_size * self.redundancy;
		req.group_size = req.group_size * self.redundancy;
		return req;
	}
	fn downward_size(&self,top_size:usize,bottom_group_size:usize,bottom_level:usize,height:usize) -> Result<usize,Error>
	{
		let base_downward_size = self.base.downward_size(top_size/self.redundancy,bottom_group_size,bottom_level,height)?;
		//println!("downward_size {} top_size {} bottom_group_size {} bottom_level {} height {}", base_downward_size, top_size, bottom_group_size,bottom_level, height);

		Ok(base_downward_size * self.redundancy) //* self.multiplier
	}

	fn amount_to_above(&self,below_router:usize, group_size: usize, bottom_size: usize) -> usize
	{
		let base_bottom_size = bottom_size / self.redundancy; // / self.multiplier
		let base_below_router = below_router % base_bottom_size;
		//println!("amount_to_above {}", self.base.amount_to_above(base_below_router, group_size, base_bottom_size));
		self.base.amount_to_above(base_below_router, group_size, base_bottom_size) * self.redundancy
	}
	fn amount_to_below(&self,above_router:usize, group_size: usize, bottom_size: usize) -> usize
	{
		let base_bottom_size = bottom_size / self.redundancy; // / self.multiplier
		let base_above_router = above_router % base_bottom_size;
		let base_deg = self.base.amount_to_below(base_above_router,group_size,base_bottom_size);
		//println!("amount_to_below {}", base_deg);
		base_deg * self.redundancy
	}
	fn to_above(&self, below_router:usize, index:usize, group_size:usize, bottom_size: usize) -> (usize,usize)
	{
		let base_bottom_size = bottom_size / self.redundancy;
		let base_below_router = below_router % base_bottom_size;
		let below_quotient = below_router / base_bottom_size;

		let base_deg = self.base.amount_to_above(base_below_router,group_size,base_bottom_size);


		let index_quotient = index / base_deg;
		let index_remainder = index % base_deg;

		let (neighbour,rev_index) = self.base.to_above(base_below_router, index_remainder, group_size, base_bottom_size);

		let below_quotient_2 = (below_quotient + index_quotient) % self.redundancy;

		//(neighbour + base_bottom_size*index_quotient, rev_index + below_quotient_2*base_deg)
		(neighbour + base_bottom_size*below_quotient_2, rev_index + index_quotient*base_deg)
	}
	fn to_below(&self, above_router:usize, index:usize, group_size:usize, bottom_size: usize) -> (usize,usize)
	{

		let base_bottom_size = bottom_size/self.redundancy;
		let base_above_router = above_router % base_bottom_size;
		let above_quotient = above_router/base_bottom_size;

		let base_deg = self.base.amount_to_below(base_above_router,group_size,base_bottom_size);

		let index_quotient = index / base_deg;
		let index_remainder = index % base_deg;

		let (neighbour,rev_index) = self.base.to_below(base_above_router, index_remainder, group_size, base_bottom_size);


		let above_quotient_2 = (above_quotient + index_quotient) % self.redundancy;
		//(neighbour + base_bottom_size*index_quotient, rev_index + above_quotient_2*base_deg)
		(neighbour + base_bottom_size*above_quotient_2, rev_index + index_quotient*base_deg)
	}
}

impl FaultToleranceStage
{
	pub fn new(mut arg:StageBuilderArgument) -> FaultToleranceStage
	{
		let mut base=None;
		let mut redundancy=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Redundant"
			{
				panic!("A Redundant must be created from a `Redundant` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"base" => base=Some(new_stage(StageBuilderArgument{cv:value, rng:&mut arg.rng,..arg})),
					"redundancy" => match value
					{
						&ConfigurationValue::Number(f) => redundancy=Some(f as usize),
						_ => panic!("bad value for redundancy"),
					},
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in Redundant",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a Widened from a non-Object");
		}
		let base=base.expect("There were no base");
		let redundancy=redundancy.expect("There were no redundancy");
		FaultToleranceStage{
			base,
			redundancy,
		}
	}
}


///A topology made of stages. Each of the `height` stages connect two levels of routers, giving a total of `height+1` levels of routers.
///Router links are exclusively between immediate levels as provided by the stages.
///Routers at level 0 are sometimes called 'leafs' and they are the only routers connected to servers.
///Routers in the topmost level (`height+1`) are sometimes called spine, althugh this terminology is mostly used for height 1.
///It may be assumed that any leaf routers are connected by a up/down path consisting on a level-increasing subpath and a level-decreasing subpath. Then the maximum distance is at most `2height`.
///TODO: talk about grouping and how size is determined.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct MultiStage
{
	//defining:
	pub stages: Vec<Box<dyn Stage>>,
	pub servers_per_leaf: usize,
	//computed:
	pub routers_per_level: Vec<usize>,
	total_routers: usize,
	pub group_sizes: Vec<usize>,
	//up_distances: Vec<Vec<Option<usize>>>,
	//up_down_distances: Vec<Vec<Option<usize>>>,
	//up_down_distances: Vec<Vec<Option<(usize,usize)>>>,
	up_down_distances: Matrix<Option<(u8,u8)>>,
	///Distance as a flat graph. distance_matrix.get(i,j) = distance from router i to router j
	flat_distance_matrix:Matrix<u8>,
}

impl Topology for MultiStage
{
	fn num_routers(&self) -> usize
	{
		self.total_routers
	}
	fn num_servers(&self) -> usize
	{
		self.routers_per_level[0]*self.servers_per_leaf
	}
	///First upwards, then downwards. Thus being coherent with the servers last convention.
	///The link-class is k for the k-th stage, and stages.len() for the server links.
	fn neighbour(&self, router_index:usize, mut port: usize) -> (Location,usize)
	{
		let (level,offset) = self.unpack(router_index);
		if level<self.stages.len()
		{
			let deg_up = self.stages[level].amount_to_above(offset,self.group_sizes[level],self.routers_per_level[level]);
			if port<deg_up
			{
				//Go upwards
				let (neighbour_offset,neighbour_down_index)=self.stages[level].to_above(offset,port,self.group_sizes[level],self.routers_per_level[level]);
				let neighbour = self.pack(level+1,neighbour_offset);
				let neighbour_deg_up = if level+1==self.stages.len() {0} else { self.stages[level+1].amount_to_above(neighbour_offset,self.group_sizes[level+1],self.routers_per_level[level+1]) };
				return (Location::RouterPort{router_index:neighbour,router_port:neighbour_down_index + neighbour_deg_up},level);
			}
			port -= deg_up;
		}
		if level==0
		{
			(Location::ServerPort(offset*self.servers_per_leaf+port),self.stages.len())
		}
		else
		{
			//Go downwards
			let (neighbour_offset,neighbour_up_index) = self.stages[level-1].to_below(offset,port,self.group_sizes[level-1],self.routers_per_level[level-1]);
			let neighbour = self.pack(level-1,neighbour_offset);
			(Location::RouterPort{router_index:neighbour,router_port:neighbour_up_index},level-1)
		}
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let router_index = server_index/self.servers_per_leaf;
		let router_port = (server_index % self.servers_per_leaf) + self.stages[0].amount_to_above(router_index,self.group_sizes[0],self.routers_per_level[0]);
		(Location::RouterPort{
			router_index,
			router_port,
		},self.stages.len())
	}
	fn diameter(&self) -> usize
	{
		let mut diameter = 0;
		for i in 0..self.routers_per_level[0]
		{
			for z in 0..self.routers_per_level[0]
			{
				let distance = *self.flat_distance_matrix.get(i,z);
				if diameter < distance
				{
					diameter = distance;
				}
			}
		}
		diameter.into()
	}
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		//up-down distance is not defined to every pair so we cannot use it.
		//Or perhaps allow infinite / replace return in signature to Option<usize>
		//self.up_down_distances[origin][destination].unwrap_or_else(||panic!("there is no up/down path among those routers: {} to {}",origin,destination))
		(*self.flat_distance_matrix.get(origin,destination)).into()
	}
	fn amount_shortest_paths(&self, origin:usize, destination:usize) -> usize
	{
		//todo!()
		let minimal_distance = (*self).distance(origin, destination);
		let mut n_paths = 0;

		for i in 0..self.total_routers
		{
			if (*self).distance(i, destination) < minimal_distance && (*self).distance(i, origin) == 1
			{
				n_paths += 1;
			}
		}
		n_paths
	}
	fn average_amount_shortest_paths(&self) -> f32
	{
		todo!()
	}
	fn maximum_degree(&self) -> usize
	{
		(0..self.num_routers()).map(|router|self.degree(router)).max().expect("there must be some router")
	}
	fn minimum_degree(&self) -> usize
	{
		(0..self.num_routers()).map(|router|self.degree(router)).min().expect("there must be some router")
	}
	fn degree(&self, router_index: usize) -> usize
	{
		let (level,offset) = self.unpack(router_index);
		let mut deg = 0;
		if level<self.stages.len()
		{
			deg += self.stages[level].amount_to_above(offset,self.group_sizes[level],self.routers_per_level[level]);
		}
		if level>0
		{
			deg += self.stages[level-1].amount_to_below(offset,self.group_sizes[level-1],self.routers_per_level[level-1]);
		}
		deg
	}
	fn ports(&self, router_index: usize) -> usize
	{
		let (level,offset) = self.unpack(router_index);
		let mut deg = 0;
		if level<self.stages.len()
		{
			deg += self.stages[level].amount_to_above(offset,self.group_sizes[level],self.routers_per_level[level]);
		}
		if level>0
		{
			deg += self.stages[level-1].amount_to_below(offset,self.group_sizes[level-1],self.routers_per_level[level-1]);
		}
		else
		{
			deg += self.servers_per_leaf;
		}
		deg
	}
	fn cartesian_data(&self) -> Option<&CartesianData>
	{

		None
	}
	fn coordinated_routing_record(&self, _coordinates_a:&[usize], _coordinates_b:&[usize], _rng: Option<&mut StdRng>)->Vec<i32>
	{
		unimplemented!();
	}
	fn is_direction_change(&self, _router_index:usize, _input_port: usize, _output_port: usize) -> bool
	{
		true
	}
	fn up_down_distance(&self,origin:usize,destination:usize) -> Option<(usize,usize)>
	{
		//*self.up_down_distances.get(origin,destination)
		self.up_down_distances.get(origin,destination).map(|(u,d)|(u.into(),d.into()))
	}
}

impl MultiStage
{
	fn initialize(&mut self)
	{
		let height=self.stages.len();
		//Find number of routers per level.
		self.routers_per_level.resize(self.stages.len()+1,0);
		self.group_sizes.resize(self.routers_per_level.len(),0);
		let mut requirements=LevelRequirements::default();
		for stage_index in 0..self.stages.len()
		{
			let stage=&self.stages[stage_index];
			self.group_sizes[stage_index]=requirements.group_size;
			requirements = stage.compose_requirements_upward(requirements,stage_index,height);
		}
		self.group_sizes[height]=requirements.group_size;
		self.routers_per_level[height]=requirements.current_level_minimum_size;
		for stage_index in (0..self.stages.len()).rev()
		{
			let stage=&self.stages[stage_index];
			match stage.downward_size(self.routers_per_level[stage_index+1],self.group_sizes[stage_index],stage_index,height)
			{
				Ok(bottom_size) => self.routers_per_level[stage_index]=bottom_size,
				Err(_) => panic!("Could not calculate downards size in MultiStage"),
			}
		}
		//self.routers_per_level[0]=self.stages.iter().map(|s|s.below_multiplier()).product();
		//self.group_sizes[0]=1;
		//for stage_index in 0..self.stages.len()
		//{
		//	let stage=&self.stages[stage_index];
		//	self.routers_per_level[stage_index+1]=self.routers_per_level[stage_index]*stage.above_multiplier()/stage.below_multiplier();
		//	self.group_sizes[stage_index+1]=self.group_sizes[stage_index]*stage.below_multiplier();
		//	if !stage.verify(self.routers_per_level[stage_index],self.routers_per_level[stage_index+1])
		//	{
		//		panic!("MultiStage network could not be initialized: failed verification on stage {}",stage_index);
		//	}
		//}
		self.total_routers=self.routers_per_level.iter().sum();
		//dbg!(&self.group_sizes);
		//dbg!(&self.routers_per_level);
		//Build distance tables
		//For each origing an ascending BFS build the up-distances and then a descending BFS build the up-down-distances.
		//self.up_distances.resize(self.total_routers,vec![]);
		//self.up_down_distances.resize(self.total_routers,vec![]);
		self.up_down_distances=Matrix::constant(None, self.total_routers,self.total_routers);
		for origin in 0..self.total_routers
		{
			//let mut ud=vec![None;self.total_routers];
			let mut udd=vec![None;self.total_routers];
			//ud[origin]=Some(0);
			udd[origin]=Some((0,0));
			//The updwards BFS.
			for current in 0..self.total_routers
			{
				if let Some((current_up,_)) = udd[current]
				{
					let (current_stage,current_offset) = self.unpack(current);
					if current_stage<self.stages.len()
					{
						let alternate_distance = current_up + 1;
						let stage = &self.stages[current_stage];
						let group_size=self.group_sizes[current_stage];
						let level_size = self.routers_per_level[current_stage];
						let neighbour_amount = stage.amount_to_above(current_offset,group_size,level_size);
						for neighbour_index in 0..neighbour_amount
						{
							let (neighbour_offset,_) = stage.to_above(current_offset,neighbour_index,group_size,level_size);
							let neighbour = self.pack(current_stage+1,neighbour_offset);
							// If there is set any distance it must be the good one already.
							// if udd[neighbour].map_or(true,|d|alternate_distance<d)
							if udd[neighbour].is_none()
							{
								udd[neighbour]=Some((alternate_distance,0));
								//ud[neighbour]=Some(alternate_distance);
							}
						}
					}
				}
			}
			//The downwards BFS.
			for current in (0..self.total_routers).rev()
			{
				if let Some((current_up,current_down)) = udd[current]
				{
					let (current_stage,current_offset) = self.unpack(current);
					if current_stage>0
					{
						let alternate_distance = current_up + current_down + 1;
						let stage = &self.stages[current_stage-1];
						let group_size=self.group_sizes[current_stage-1];
						let level_size = self.routers_per_level[current_stage-1];
						let neighbour_amount = stage.amount_to_below(current_offset,group_size,level_size);
						for neighbour_index in 0..neighbour_amount
						{
							let (neighbour_offset,_) = stage.to_below(current_offset,neighbour_index,group_size,level_size);
							let neighbour = self.pack(current_stage-1,neighbour_offset);
							// Now some distances can be lesser than the new, so we need the whole check.
							//if udd[neighbour].map_or(true,|d|alternate_distance<d)
							//{
							//	// Only update the up_down_distance.
							//	udd[neighbour]=Some(alternate_distance);
							//}
							if udd[neighbour].map_or(true,|(u,d)|alternate_distance<u+d)
							{
								// Only update the up_down_distance.
								udd[neighbour]=Some((current_up,current_down+1));
							}
						}
					}
				}
			}
			//self.up_distances[origin]=ud;
			//self.up_down_distances[origin]=udd;
			for i in 0..self.total_routers
			{
				*self.up_down_distances.get_mut(origin,i) = udd[i];
			}
		}
		//And the flat distances
		//self.flat_distance_matrix=self.compute_distance_matrix(None);
		self.flat_distance_matrix=self.compute_distance_matrix(None).map(|entry|*entry as u8);
	}
	///Unpacks a router giving the level (by index) and its position in that stage.
	///Only valid when routers_per_level has been already computed.
	///It is the inverse of `pack`.
	pub fn unpack(&self, router:usize) -> (usize,usize)
	{
		let mut level_index=0;
		let mut offset=router;
		while offset>=self.routers_per_level[level_index]
		{
			offset-=self.routers_per_level[level_index];
			level_index+=1;
		}
		(level_index,offset)
	}
	///Return the router index giving its level (distance to a leaf) and offset (poisition in such level).
	///It is the inverse of `unpack`.
	pub fn pack(&self, level_index:usize, offset:usize) -> usize
	{
		offset + self.routers_per_level.iter().take(level_index).sum::<usize>()
	}
	pub fn new(arg:TopologyBuilderArgument) -> MultiStage
	{
		let stages;
		let mut servers_per_leaf=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			match cv_name.as_ref()
			{
				"MultiStage" =>
				{
					let mut got_stages = None;
					for &(ref name,ref value) in cv_pairs
					{
						match name.as_ref()
						{
							"stages" => match value
							{
								&ConfigurationValue::Array(ref a) => got_stages=Some(a.iter().map(|cv|new_stage(StageBuilderArgument{cv,plugs:arg.plugs,rng:arg.rng})).collect()),
								_ => panic!("bad value for stages"),
							},
							"servers_per_leaf" => match value
							{
								&ConfigurationValue::Number(f) => servers_per_leaf=Some(f as usize),
								_ => panic!("bad value for servers_per_leaf"),
							},
							"legend_name" => (),
							_ => panic!("Nothing to do with field {} in MultiStage",name),
						}
					}
					stages=got_stages.expect("There were no stages");
				},
				"XGFT" =>
				{
					let mut height=None;
					let mut down:Option<Vec<usize>>=None;
					let mut up:Option<Vec<usize>>=None;
					for &(ref name,ref value) in cv_pairs
					{
						match name.as_ref()
						{
							"height" => match value
							{
								&ConfigurationValue::Number(f) => height=Some(f as usize),
								_ => panic!("bad value for height"),
							},
							"down" => match value
							{
								//&ConfigurationValue::Number(f) => down=Some(f as usize),
								&ConfigurationValue::Array(ref a) => down=Some(a.iter().map(|v|match v{
									&ConfigurationValue::Number(f) => f as usize,
									_ => panic!("bad value in down"),
								}).collect()),
								_ => panic!("bad value for down"),
							},
							"up" => match value
							{
								&ConfigurationValue::Array(ref a) => up=Some(a.iter().map(|v|match v{
									&ConfigurationValue::Number(f) => f as usize,
									_ => panic!("bad value in up"),
								}).collect()),
								_ => panic!("bad value for up"),
							},
							"servers_per_leaf" => match value
							{
								&ConfigurationValue::Number(f) => servers_per_leaf=Some(f as usize),
								_ => panic!("bad value for servers_per_leaf"),
							},
							"legend_name" => (),
							_ => panic!("Nothing to do with field {} in XGFT",name),
						}
					}
					let height=height.expect("There were no height");
					let down=down.expect("There were no down");
					let up=up.expect("There were no up");
					if height!=down.len()
					{
						panic!("down does not match length");
					}
					if height!=up.len()
					{
						panic!("up does not match length");
					}
					stages=(0..height).map(|index|Box::new(FatStage{bottom_factor:down[index],top_factor:up[index]}) as Box<dyn Stage>).collect();
				}
				"OFT" =>
				{
					let mut height=None;
					let mut prime=None;
					let mut double_topmost_level = true;
					let mut redundant = 1;
					for &(ref name,ref value) in cv_pairs
					{
						match name.as_ref()
						{
							"height" => match value
							{
								&ConfigurationValue::Number(f) => height=Some(f as usize),
								_ => panic!("bad value for height"),
							},
							"prime" => match value
							{
								&ConfigurationValue::Number(f) => prime=Some(f as usize),
								_ => panic!("bad value for prime"),
							},
							"servers_per_leaf" => match value
							{
								&ConfigurationValue::Number(f) => servers_per_leaf=Some(f as usize),
								_ => panic!("bad value for servers_per_leaf"),
							},
							"double_topmost_level" => match value
							{
								&ConfigurationValue::True => double_topmost_level=true,
								&ConfigurationValue::False => double_topmost_level=false,
								_ => panic!("bad value for double_topmost_level"),
							},
							"redundant" => match value
							{
								&ConfigurationValue::Number(f) => redundant=f as usize,
								_ => panic!("bad value for redundant"),
							},
							"legend_name" => (),
							_ => panic!("Nothing to do with field {} in OFT",name),
						}
					}
					let height=height.expect("There were no height");
					let prime=prime.expect("There were no prime");
					// let redundant = redundant.expect("There was no redundant");

					stages=(0..height).map(|index|{
						let stage=ProjectiveStage{
							//This is somewhat repetitive...
							plane:FlatGeometryCache::new_prime(prime).unwrap_or_else(|_|panic!("{} is not prime, which is required for the OFT topology",prime)),
						};
						if double_topmost_level && index+1==height && redundant == 1
						{
							Box::new(WidenedStage{ base:Box::new(stage), multiplier:2 }) as Box<dyn Stage>

						} else if redundant > 1 {

							Box::new(FaultToleranceStage{ base:Box::new(stage), redundancy:redundant }) as Box<dyn Stage>

						} else {
							Box::new(stage) as Box<dyn Stage>
						}
						//Box::new(stage) as Box<dyn Stage>
					}).collect();
				}
				"MLFM" =>
				{

					let mut k=None;
					let mut pods=None;
					//let mut servers_per_leaf=None;

					for &(ref name,ref value) in cv_pairs
					{
						match name.as_ref()
						{
							"k" => match value
							{
								&ConfigurationValue::Number(f) => k=Some(f as usize),
								_ => panic!("bad value for k"),
							},
							"pods" => match value
							{
								&ConfigurationValue::Number(f) => pods=Some(f as usize),
								_ => panic!("bad value for pods"),
							},
							"servers_per_leaf" => match value
							{
								&ConfigurationValue::Number(f) => servers_per_leaf=Some(f as usize),
								_ => panic!("bad value for servers_per_leaf"),
							},
							"legend_name" => (),
							_ => panic!("Nothing to do with field {} in MLFM",name),
						}
					}

					let k=k.expect("There were no k");
					let pods=pods.expect("There were no pods");
					//let servers_per_leaf=servers_per_leaf.expect("Not working");

					let stage = FMStage{
						layers: k,
						layer_size: pods,
					};
					stages = vec![Box::new(stage) as Box<dyn Stage>];
					//stages=
					/*
					(0..height).map(|index|{
						let stage=ProjectiveStage{
							//This is somewhat repetitive...
							plane:FlatGeometryCache::new_prime(prime).unwrap_or_else(|_|panic!("{} is not prime, which is required for the OFT topology",prime)),
						};
						if double_topmost_level && index+1==height && redundant == 1
						{
							Box::new(WidenedStage{ base:Box::new(stage), multiplier:2 }) as Box<dyn Stage>

						} else if redundant > 1 {

							Box::new(FaultToleranceStage{ base:Box::new(stage), redundancy:redundant }) as Box<dyn Stage>

						} else {
							Box::new(stage) as Box<dyn Stage>
						}
						//Box::new(stage) as Box<dyn Stage>
					}).collect();*/
				}
				"RFC" =>
				{
					let mut height=None;
					let mut sizes:Option<Vec<usize>>=None;
					let mut down:Option<Vec<usize>>=None;
					let mut up:Option<Vec<usize>>=None;
					for &(ref name,ref value) in cv_pairs
					{
						match name.as_ref()
						{
							"height" => match value
							{
								&ConfigurationValue::Number(f) => height=Some(f as usize),
								_ => panic!("bad value for height"),
							},
							"sizes" => match value
							{
								//&ConfigurationValue::Number(f) => sizes=Some(f as usize),
								&ConfigurationValue::Array(ref a) => sizes=Some(a.iter().map(|v|match v{
									&ConfigurationValue::Number(f) => f as usize,
									_ => panic!("bad value in sizes"),
								}).collect()),
								_ => panic!("bad value for sizes"),
							},
							"down" => match value
							{
								//&ConfigurationValue::Number(f) => down=Some(f as usize),
								&ConfigurationValue::Array(ref a) => down=Some(a.iter().map(|v|match v{
									&ConfigurationValue::Number(f) => f as usize,
									_ => panic!("bad value in down"),
								}).collect()),
								_ => panic!("bad value for down"),
							},
							"up" => match value
							{
								&ConfigurationValue::Array(ref a) => up=Some(a.iter().map(|v|match v{
									&ConfigurationValue::Number(f) => f as usize,
									_ => panic!("bad value in up"),
								}).collect()),
								_ => panic!("bad value for up"),
							},
							"servers_per_leaf" => match value
							{
								&ConfigurationValue::Number(f) => servers_per_leaf=Some(f as usize),
								_ => panic!("bad value for servers_per_leaf"),
							},
							"legend_name" => (),
							_ => panic!("Nothing to do with field {} in RFC",name),
						}
					}
					let height=height.expect("There were no height");
					let sizes=sizes.expect("There were no sizes");
					let down=down.expect("There were no down");
					let up=up.expect("There were no up");
					if height!=down.len()
					{
						panic!("down does not match length");
					}
					if height!=up.len()
					{
						panic!("up does not match length");
					}
					if height+1!=sizes.len()
					{
						panic!("sizes does not match length+1");
					}
					stages=(0..height).map(|index|{
						let bottom_size=sizes[index];
						let top_size=sizes[index+1];
						let (upwards,downwards) = ExplicitStage::random_adjacencies(bottom_size,up[index],top_size,down[index],arg.rng);
						let (bottom_list,top_list) = ExplicitStage::add_reverse_indices(&upwards,&downwards);
						let stage=ExplicitStage{bottom_size,top_size,bottom_list,top_list};
						Box::new(stage) as Box<dyn Stage>
					}).collect();
				}
			_ => panic!("Cannot create a MultiStage from a `{}` object",cv_name),
			}
		}
		else
		{
			panic!("Trying to create a MultiStage from a non-Object");
		}
		let servers_per_leaf=servers_per_leaf.expect("There were no servers_per_leaf");
		let mut network = MultiStage{
			stages,
			servers_per_leaf,
			routers_per_level: vec![],
			total_routers:0,
			group_sizes: vec![],
			//up_distances: vec![],
			up_down_distances: Matrix::constant(None,0,0),
			flat_distance_matrix: Matrix::constant(0,0,0),
		};
		network.initialize();
		network
	}
}

pub struct StageBuilderArgument<'a>
{
	///A ConfigurationValue::Object defining the topology.
	pub cv: &'a ConfigurationValue,
	///The user defined plugs. In case the topology needs to create elements.
	pub plugs: &'a Plugs,
	///The random number generator to use.
	pub rng: &'a mut StdRng,
}

/**
Build a new Stage, intended as part of a multistage network.


### Fat-tree stage
This is a full connectivity over the involved groups.
```ignore
Fat{
	bottom_factor: 4,
	top_factor: 4,
}
```

### Projective stage
A stage following the connectivity in a Orthogonal Fat-Tree (OFT).
```ignore
Projective{
	prime: 3,
}
```

### Randomly interconnected stage
```ignore
RandomRegular{
	bottom_size: 32,
	top_size: 16,
	upwards_degree:4,
	downwards_degree:8,
}
```

### Widened stage operation
This modifies a given stage by indicating that should be `multiplier` times more routers at the bottom. With `multiplier=2`, when used in the last stage, can be interpreted as using all ports in the topmost level downwards, therefore, doubling the downwards degree and the number of routers at the bottom. It is of no use for `Fat` or `RandomRegular` stages, but it is useful for the `Projective` stage. Indeed, it is employed internally when building directly a `OFT` topology.
```ignore
Widened{
	base: Projective { prime:3 },
	multiplier:2,
}
```


*/
pub fn new_stage(arg:StageBuilderArgument) -> Box<dyn Stage>
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
	{
		if let Some(builder) = arg.plugs.stages.get(cv_name)
		{
			return builder(arg);
		}
		match cv_name.as_ref()
		{
			"Fat" => Box::new(FatStage::new(arg)),
			"Projective" => Box::new(ProjectiveStage::new(arg)),
			"RandomRegular" => Box::new(ExplicitStage::new(arg)),
			"ExplicitStageFile" => Box::new(ExplicitStageFile::new(arg)),
			"Widened" => Box::new(WidenedStage::new(arg)),
			"FaultToleranceStage" => Box::new(FaultToleranceStage::new(arg)),
			"FMStage" => Box::new(FMStage::new(arg)),
			_ => panic!("Unknown stage {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create a stage from a non-Object");
	}
}


/**

Routing for indirect networks which follows up-down routes adaptively.
It asumes there's an up-down path to destination from any leaf.

```ignore
	UpDownDerouting{
		allowed_updowns: 2 // 1 Non-min + 1 Min
		virtual_channels: [[0, 1], [2, 3]], // 2 phases, 2 VC per phases.
		stages: 1. // 1 stage in the multistage
	}
```
**/
#[derive(Debug)]
pub struct UpDownDerouting
{
	///Number of up-down stages allowed.
	allowed_updowns: usize,
	/// (Optional): VC to take in each UpDown stage. By default one different VC per UpDown path.
	virtual_channels: Vec<Vec<usize>>,
	/// Stages in the multistage, by the default 1.
	stages: usize,
}

impl Routing for UpDownDerouting
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		let distance=topology.distance(current_router,target_router);
		if distance==0
		{
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server.expect("Expect a server")
					{
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}

		let avaliable_updown_deroutes = routing_info.selections.as_ref().unwrap()[0] as usize; //Avaliable up-down phases. If its 1, we are in the last updown phase.

		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		let vc_index= self.allowed_updowns - avaliable_updown_deroutes; // To know the updown phase we are in.

		for NeighbourRouterIteratorItem{link_class: _next_link_class,port_index,neighbour_router:neighbour_router_index,..} in topology.neighbour_router_iter(current_router)
		{
			if distance-1 == topology.distance(neighbour_router_index,target_router) //Minimal route always welcomed
			{
				r.extend(self.virtual_channels[vc_index].iter().map(|&vc|CandidateEgress::new(port_index,vc)));

			}else if avaliable_updown_deroutes > 1{ // a non-minimal route shouldnt be allowed if we only have 1 updown deroute left. We asume that there's an updown path to destination.

				r.extend(self.virtual_channels[vc_index].iter().map(|&vc|CandidateEgress{port:port_index,virtual_channel:vc,label:1,..Default::default()}));

			}
		}
		Ok(RoutingNextCandidates{candidates:r,idempotent:true})
	}

	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, current_router:usize, _target_touter:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		routing_info.borrow_mut().selections=Some(vec![self.allowed_updowns as i32]);
		routing_info.borrow_mut().visited_routers=Some(vec![current_router]);
		routing_info.borrow_mut().auxiliar= RefCell::new(Some(Box::new(vec![0usize;self.stages])));

	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, _target_server:Option<usize>,_rng: &mut StdRng)
	{

		if let (Location::RouterPort{router_index: _previous_router,router_port:_},link_class)=topology.neighbour(current_router,current_port)
		{
			//let multistage_data = topology.cartesian_data().expect("To run UpDownDerouting you need a multistage topology!");
			//let max_height = multistage_data.height();
			//let current_height= multistage_data.unpack(current_router)[0];

			let mut bri=routing_info.borrow_mut();
			let aux = bri.auxiliar.borrow_mut().take().unwrap();
			let mut saltos =  aux.downcast_ref::<Vec<usize>>().unwrap().clone();
			if saltos[link_class] != 0
			{
				saltos[link_class] = 0usize;
				if link_class == 0  && current_router != target_router// now we are in last stage
				{
					match bri.selections
					{
						Some(ref mut v) =>
							{
								let available_updown_deroutes=v[0];
								if available_updown_deroutes==0
								{
									panic!("Bad deroute :(");
								}
								v[0]= available_updown_deroutes-1;
							}
						None => panic!("selections not initialized"),
					};
				}
			}else{
				saltos[link_class] = 1usize;
			}

			bri.auxiliar.replace(Some(Box::new(saltos)));

			match bri.visited_routers
			{
				Some(ref mut v) =>
				{
					v.push(current_router);
				}
				None => panic!("visited_routers not initialized"),
			};

		}
	}
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &mut StdRng)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _num_virtual_channels:usize, _rng:&mut StdRng)
	{
	}
	fn statistics(&self, _cycle:Time) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:Time)
	{
	}
}

impl UpDownDerouting
{
	pub fn new(arg:RoutingBuilderArgument) -> UpDownDerouting
	{
		let mut allowed_updowns=None;
		let mut stages = 1usize;
		let mut virtual_channels = None;

		//let mut include_labels=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="UpDownDerouting"
			{
				panic!("A UpDownDerouting must be created from a `UpDownDerouting` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"allowed_updowns" => match value
					{
						&ConfigurationValue::Number(f) => allowed_updowns =Some(f as usize),
						_ => panic!("bad value for allowed_deroutes"),
					}
					"stages" => match value {
						&ConfigurationValue::Number(f) => stages = f as usize,
						_ => (),
					}
					"virtual_channels" => match value {
						ConfigurationValue::Array(f) => virtual_channels = Some(f.into_iter().map(| a | a.as_array().unwrap().into_iter().map(|b| b.as_usize().unwrap()).collect() ).collect()),
						_ => (),
					}
					/*"include_labels" => match value
					{
						&ConfigurationValue::True => include_labels=Some(true),
						&ConfigurationValue::False => include_labels=Some(false),
						_ => panic!("bad value for include_labels"),
					}*/
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in UpDownDerouting",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a UpDownDerouting from a non-Object");
		}
		let allowed_updowns= allowed_updowns.expect("There were no allowed_deroutes");

		let virtual_channels = match virtual_channels {
			Some( v) => v,
			None => {
				let a= vec![0;allowed_updowns];
				a.iter().enumerate().map(|(i,_vc)|vec![i]).collect::<Vec<Vec<usize>>>()
			}
		};


		//let include_labels=include_labels.expect("There were no include_labels");
		UpDownDerouting {
			allowed_updowns,
			virtual_channels,
			stages
		}
	}
}

use std::mem::{size_of,size_of_val};
use std::collections::{VecDeque,BTreeMap,BTreeSet,HashMap};
use std::rc::Rc;
use std::cell::{RefCell};
//use quantifiable_derive::Quantifiable;//the derive macro
//use ::quantifiable_derive::quantifiable_macro_derive;//the derive macro

// See https://users.rust-lang.org/t/deriving-the-implementation-of-trait-for-structs/25730/9
// This is similar to https://docs.rs/heapsize/0.4.2/heapsize/ as noted by notriddle
// heapsize derive macro is at https://docs.rs/heapsize_derive/0.1.4/src/heapsize_derive/lib.rs.html#5-108 , as pointed by droundy

pub trait Quantifiable
{
	/// Get the total memory currently being employed by the implementing type. Both stack and heap.
	fn total_memory(&self) -> usize;
	/// Prints by stdout how much memory is used per component.
	fn print_memory_breakdown(&self);
	/// Get an estimation on how much memory the type could reach during the simulation.
	fn forecast_total_memory(&self) -> usize;
}

impl<T:Quantifiable> Quantifiable for Vec<T>
{
	fn total_memory(&self) -> usize
	{
		size_of::<Vec<T>>() + self.iter().map(|e|e.total_memory()).sum::<usize>() + (self.capacity()-self.len())*size_of::<T>()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<K,V> Quantifiable for HashMap<K,V>
{
	fn total_memory(&self) -> usize
	{
		//TODO...
		0
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<A:Quantifiable, B:Quantifiable> Quantifiable for (A,B)
{
	fn total_memory(&self) -> usize
	{
		self.0.total_memory()+self.1.total_memory()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<A:Quantifiable, B:Quantifiable, C:Quantifiable> Quantifiable for (A,B,C)
{
	fn total_memory(&self) -> usize
	{
		self.0.total_memory()+self.1.total_memory()+self.2.total_memory()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<A:Quantifiable, B:Quantifiable, C:Quantifiable, D:Quantifiable> Quantifiable for (A,B,C,D)
{
	fn total_memory(&self) -> usize
	{
		self.0.total_memory()+self.1.total_memory()+self.2.total_memory()+self.3.total_memory()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<A:Quantifiable, B:Quantifiable, C:Quantifiable, D:Quantifiable, E:Quantifiable> Quantifiable for (A,B,C,D,E)
{
	fn total_memory(&self) -> usize
	{
		self.0.total_memory()+self.1.total_memory()+self.2.total_memory()+self.3.total_memory()+ self.4.total_memory()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<T:Quantifiable> Quantifiable for [T;2]
{
	fn total_memory(&self) -> usize
	{
		self[0].total_memory()+self[1].total_memory()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

macro_rules! quantifiable_simple
{
	($t:ty) =>
	{
		impl Quantifiable for $t
		{
			fn total_memory(&self) -> usize
			{
				size_of::<$t>()
			}
			fn print_memory_breakdown(&self)
			{
				unimplemented!();
			}
			fn forecast_total_memory(&self) -> usize
			{
				size_of::<$t>()
			}
		}
	}
}

quantifiable_simple!(bool);
quantifiable_simple!(u8);
quantifiable_simple!(i32);
quantifiable_simple!(u64);
quantifiable_simple!(u128);
quantifiable_simple!(usize);
quantifiable_simple!(String);
//impl Quantifiable for usize
//{
//	fn total_memory(&self) -> usize
//	{
//		return size_of::<usize>();
//	}
//	fn print_memory_breakdown(&self)
//	{
//		unimplemented!();
//	}
//	fn forecast_total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//}

quantifiable_simple!(f32);
quantifiable_simple!(f64);
//impl Quantifiable for f32
//{
//	fn total_memory(&self) -> usize
//	{
//		return size_of::<f32>();
//	}
//	fn print_memory_breakdown(&self)
//	{
//		unimplemented!();
//	}
//	fn forecast_total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//}

impl<T> Quantifiable for *const T
{
	fn total_memory(&self) -> usize
	{
		size_of::<Self>()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<T:Quantifiable> Quantifiable for VecDeque<T>
{
	fn total_memory(&self) -> usize
	{
		//return size_of::<VecDeque<T>>() + self.capacity()*size_of::<T>();
		size_of::<VecDeque<T>>() + self.iter().map(|e|e.total_memory()).sum::<usize>() + (self.capacity()-self.len())*size_of::<T>()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<A:Quantifiable, B:Quantifiable> Quantifiable for BTreeMap<A,B>
{
	fn total_memory(&self) -> usize
	{
		size_of::<Self>() + self.len()*(size_of::<A>()+size_of::<B>())
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<A:Quantifiable> Quantifiable for BTreeSet<A>
{
	fn total_memory(&self) -> usize
	{
		size_of::<Self>() + self.len()*size_of::<A>()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<T:?Sized> Quantifiable for Rc<T>
{
	fn total_memory(&self) -> usize
	{
		size_of::<Rc<T>>()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<T:Quantifiable+?Sized> Quantifiable for Box<T>
{
	fn total_memory(&self) -> usize
	{
		size_of::<Box<T>>() + T::total_memory(self)
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<T:Quantifiable> Quantifiable for Option<T>
{
	fn total_memory(&self) -> usize
	{
		//return size_of::<Box<T>>() + T::total_memory(self);
		match self
		{
			&None => size_of::<Option<T>>(),
			&Some(ref thing) => thing.total_memory(),
		}
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

impl<T:Quantifiable+?Sized> Quantifiable for RefCell<T>
{
	fn total_memory(&self) -> usize
	{
		//Note: BorrowFlag=isize (at 15/3/2019)
		//self.borrow().total_memory() + size_of::<Cell<isize>>()
		self.borrow().total_memory() + size_of_val(self)
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

/// TODO: calculate something here.
impl Quantifiable for rand::rngs::StdRng
{
	fn total_memory(&self) -> usize { 0 }
	fn print_memory_breakdown(&self) { }
	fn forecast_total_memory(&self) -> usize { 0 }
}


impl Quantifiable for dyn std::any::Any
{
	fn total_memory(&self) -> usize
	{
		size_of::<&dyn std::any::Any>()
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

pub fn human_bytes(byte_amount:usize) -> String
{
	if byte_amount<3000
	{
		return format!("{} bytes",byte_amount);
	}
	let kb_amount=byte_amount as f64 / 1024.;
	if kb_amount<3000f64
	{
		return format!("{} KB",kb_amount);
	}
	let mb_amount=kb_amount / 1024.;
	if mb_amount<3000f64
	{
		return format!("{} MB",mb_amount);
	}
	let gb_amount=mb_amount / 1024.;
	return format!("{} GB",gb_amount);
}





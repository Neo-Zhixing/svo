use crate::arena::NodeSlot;
use crate::{Arena, ArenaHandle, Octree, Voxel};
use std::collections::{HashMap, VecDeque};
use std::io::{BufWriter, Read, Write};
use std::mem::size_of;
use std::slice::{from_raw_parts, from_raw_parts_mut};

impl<T: Voxel> Octree<T> {
    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Writing some metadata
        unsafe {
            writer.write(from_raw_parts(
                &self.root_data as *const T as *const u8,
                size_of::<T>(),
            ))?;
        }
        // starting to DFS
        let mut queue: VecDeque<(ArenaHandle<T>, u8)> = VecDeque::new(); // todo: optimize this with_capacity
        let mut current_index: u32 = 1; // The file address of the next available slot.
        queue.push_back((self.root, 1));
        while !queue.is_empty() {
            let (nodes, num_of_children) = queue.pop_front().unwrap();
            for i in 0..num_of_children {
                // For each children of this block
                let node = nodes.offset(i as u32);
                let node_ref = &self.arena[node];

                // Write the node into the file.
                unsafe {
                    // We're having three write() calls because we want to write something
                    // different as the children index.
                    writer.write(from_raw_parts::<u8>(&node_ref.freemask, size_of::<u8>()))?;

                    if node_ref.freemask != 0 {
                        let child_block_size = node_ref.freemask.count_ones();
                        // non leaf node.
                        // Add the children of the current node to the queue.
                        queue.push_back((node_ref.children, child_block_size as u8));
                        // Translate the child index into the file space
                        writer.write(from_raw_parts::<u8>(
                            &current_index as *const u32 as *const u8,
                            size_of::<u32>(),
                        ))?;
                        current_index += child_block_size;
                    }

                    writer.write(from_raw_parts::<u8>(
                        &node_ref.data as *const T as *const u8,
                        size_of::<[T; 8]>(),
                    ))?;
                }
            }
        }
        Ok(())
    }

    pub fn read<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut arena: Arena<T> = Arena::new();
        let mut octree = Octree {
            arena,
            root: ArenaHandle::new(0, 0),
            root_data: Default::default(),
        };
        unsafe {
            // Read the root data
            reader.read_exact(from_raw_parts_mut(
                &mut octree.root_data as *mut T as *mut u8,
                size_of::<T>(),
            ));
        }

        // Mapping from file-space indices to (Parent, BlockSize)
        let mut block_size_map: VecDeque<(ArenaHandle<T>, u8)> = VecDeque::new(); // todo: optimize with_capacity
                                                                                  // let mut block_size_map: AHashMap<u32, (ArenaHandle<T>, u8)> = AHashMap::new();
                                                                                  // The root node is always the first one in the file, and the block size of the root node
                                                                                  // is always one.
        block_size_map.push_back((ArenaHandle::none(), 1));
        while !block_size_map.is_empty() {
            let (parent_handle, block_size) = block_size_map.pop_front().unwrap();

            let block = octree.arena.alloc(block_size as u32);
            if !parent_handle.is_none() {
                // Has a parent. Set the parent's child index to convert it back into memory space
                let parent_ref = &mut octree.arena[parent_handle];
                parent_ref.children = block;
            }

            for i in 0..block_size {
                let node = block.offset(i as u32);
                let node_ref = &mut octree.arena[node];
                node_ref.block_size = block_size;
                unsafe {
                    // Read the entire thing into the newly allocated node
                    reader.read_exact(from_raw_parts_mut::<u8>(
                        &mut node_ref.freemask,
                        size_of::<u8>(),
                    ))?;
                    if node_ref.freemask != 0 {
                        // has children
                        reader.read_exact(from_raw_parts_mut::<u8>(
                            &mut node_ref.children as *mut ArenaHandle<T> as *mut u8,
                            size_of::<ArenaHandle<T>>(),
                        ))?;
                        block_size_map.push_back((node, node_ref.freemask.count_ones() as u8));
                    }
                    reader.read_exact(from_raw_parts_mut::<u8>(
                        &mut node_ref.data as *mut T as *mut u8,
                        size_of::<[T; 8]>(),
                    ))?;
                }
            }
        }
        Ok((octree))
    }
}

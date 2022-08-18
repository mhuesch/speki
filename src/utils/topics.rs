use crate::utils::structs::StatefulList;
use rusqlite::Connection;
use crate::utils::sql::update::{update_topic_relpos, update_topic_parent, update_card_topic};
use crate::utils::sql::delete::{delete_topic};

#[derive(Clone)]
pub struct Topic{
    pub id: u32,
    pub name: String,
    pub parent: u32,
    pub children: Vec<u32>,
    pub ancestors: u32,
    pub relpos: u32,
}






impl StatefulList<Topic>{


pub fn is_last_sibling(&self, id: u32)-> bool {
    let relpos = self.topic_from_id(id).relpos;
    let sibling_qty = self.siblings_from_id(id).len() as u32;
    
    relpos == sibling_qty - 1
}
    
pub fn add_kids(&mut self){
   // for topic in self.items{
    //    topic.children.clear();}

    let item_clone = self.items.clone();
    let mut parentidx_childid = Vec::<(usize, u32)>::new();

    for (parent_idx, topic) in item_clone.iter().enumerate(){
        let parent_id = topic.id;
        for child in &self.items{
            if child.parent == parent_id{
                parentidx_childid.push((parent_idx, child.id));
                }
            }
        }

    for pair in parentidx_childid{
        let parent_index = pair.0;
        let child_id = pair.1;
        self.items[parent_index].children.push(child_id);
    }
    
    }


pub fn get_selected_id(&self) -> Option<u32>{
    match self.state.selected(){
        None => None,
        Some(idx) => Some(self.items[idx as usize].id)
    }
}

pub fn get_selected_parent(&self) -> Option<u32>{
    match self.state.selected(){
        None => None,
        Some(idx) => Some(self.items[idx as usize].parent)
    }
}

pub fn index_from_id(&self, id: u32) -> u32{

    for (index, topic) in self.items.iter().enumerate(){
        if topic.id == id{
            return index as u32;
        }
    }
    panic!("no matching index for id: {}", id);
}

pub fn topic_from_index(&self, index: u32) -> Topic{
    self.items[index as usize].clone()
}

pub fn topic_from_id(&self, id: u32)-> Topic{
    let index = self.index_from_id(id);
    self.topic_from_index(index)
}

pub fn parent_from_index(&self, index: u32) -> Topic{
    let topic = self.topic_from_index(index);
    self.topic_from_id(topic.parent)
}

pub fn parent_from_id(&self, id: u32) -> Topic{
    let topic = self.topic_from_id(id);
    self.topic_from_id(topic.parent)
}

pub fn grandparent_from_id(&self, id: u32)-> Topic{
    let parent = self.parent_from_id(id);
    self.parent_from_id(parent.id)
}

pub fn uncles_from_id(&self, id: u32) -> Vec<u32> {
    let grandparent = self.grandparent_from_id(id);
    let mut uncles = grandparent.children.clone();
    uncles.sort_unstable_by_key(|topid| self.items[self.index_from_id(*topid) as usize].relpos);
    uncles
}

pub fn siblings_from_id(&self, id: u32) -> Vec<u32> {
    let parent = self.parent_from_id(id);
    let mut siblings = parent.children;
    siblings.sort_unstable_by_key(|topid| self.items[self.index_from_id(*topid) as usize].relpos);
    siblings
}

pub fn ancestor_from_id(&self, id: u32, ancestors: u32) -> Topic {
    let mut topic = self.topic_from_id(id);
    
    for _ in 0..ancestors{
        topic = self.topic_from_id(topic.parent);
    }
    topic
}


pub fn sibling_below(&self, id: u32)->Topic{
    let topic = self.topic_from_id(id);
    let siblings = self.siblings_from_id(id);
    self.topic_from_id(siblings[(topic.relpos + 1) as usize])
}
pub fn sibling_above(&self, id: u32)->Topic{
    let topic = self.topic_from_id(id);
    let siblings = self.siblings_from_id(id);
    self.topic_from_id(siblings[(topic.relpos - 1) as usize])
}

pub fn distance_sibling_above(&self, id: u32) -> u32{
    let topic = self.topic_from_id(id);
    let above_sibling = self.sibling_above(id);
    let current_index = self.index_from_id(topic.id);
    let above_index = self.index_from_id(above_sibling.id);
    current_index - above_index
}

pub fn distance_sibling_below(&self, id: u32) -> u32{
    let topic = self.topic_from_id(id);
    let below_sibling = self.sibling_below(id);
    let current_index = self.index_from_id(topic.id);
    let below_index = self.index_from_id(below_sibling.id);
    below_index - current_index
}

pub fn shift_left(&mut self, conn: &Connection, index: u32){
    let topic = self.topic_from_index(index);
    let parent = self.parent_from_id(topic.id);
    let uncles = self.uncles_from_id(topic.id);
    let uncle_qty = uncles.len() as u32;
    
    update_topic_parent(conn, topic.id, parent.parent).unwrap();
    update_topic_relpos(conn, topic.id, parent.relpos).unwrap();

    for i in parent.relpos..uncle_qty{
        let uncle_id = uncles[i as usize];
        let uncle = self.items[self.index_from_id(uncle_id) as usize].clone();
        update_topic_relpos(conn, uncle_id, uncle.relpos + 1).unwrap();
    }
    let siblings = self.siblings_from_id(topic.id);
    let sibling_qty = siblings.len() as u32;

    for i in (topic.relpos + 1)..sibling_qty{
        let sibling = self.topic_from_id(siblings[i as usize]);
        update_topic_relpos(conn, siblings[i as usize], sibling.relpos - 1).unwrap();

    }

}

// TODO fix the relpos bugs
pub fn delete_topic(&mut self, conn: &Connection, index: u32){
    let topic = self.topic_from_index(index);

    for (index, child) in topic.children.iter().enumerate(){
        update_topic_parent(conn, *child, topic.parent).unwrap(); 
        update_topic_relpos(conn, *child, topic.relpos + index as u32).unwrap();
    }

    delete_topic(conn, topic.id).unwrap();
    update_card_topic(conn, topic.id, topic.parent).unwrap(); // all the cards with the deleted topic
                                                           // get assigned to the topic above item  

    let siblings = self.siblings_from_id(topic.id);
    let siblingqty = siblings.len() as u32;
    let kidqty = topic.children.len() as u32;

    for i in (topic.relpos + 1)..(siblingqty){
        update_topic_relpos(conn, siblings[(i) as usize],  i + kidqty - 1).unwrap();
    }
}


pub fn shift_right(&mut self, conn: &Connection, index: u32){
    let topic = self.topic_from_index(index);
    let below = self.topic_from_index(index + 1);
    update_topic_parent(conn, topic.id, below.id).unwrap();
    update_topic_relpos(conn, topic.id, 0).unwrap();


    for child_id in below.children{
        let child = self.topic_from_id(child_id);
        update_topic_relpos(conn, child_id, child.relpos + 1).unwrap();
    }
    let siblings = self.siblings_from_id(topic.id);
    let sibling_qty = siblings.len() as u32;
    
    for i in (topic.relpos + 1)..sibling_qty{
        let sib = self.topic_from_id(siblings[i as usize]);
        update_topic_relpos(conn, sib.id, sib.relpos - 1).unwrap();
    }
}

pub fn shift_down(&mut self, conn: &Connection, index: u32){
    let topic = self.topic_from_index(index);
    let siblings = self.siblings_from_id(topic.id);
    let below_sibling = self.sibling_below(topic.id);
    let sibling_qty = siblings.len() as u32;
    
    // if topic is not the last relpos, shift its relpos one down and the below it one up 
    
    if topic.relpos != sibling_qty - 1 {
        update_topic_relpos(conn, topic.id, topic.relpos + 1).unwrap();
        update_topic_relpos(conn, below_sibling.id, topic.relpos).unwrap();
        return
    } 
}

pub fn shift_up(&mut self, conn: &Connection, index: u32){
    let topic = self.items[index as usize].clone();
    let topic_id = self.get_selected_id().unwrap();
    let relpos = topic.relpos;
    let parent_id = topic.parent;
    if parent_id == 1 && relpos == 0{return};

    

    if relpos == 0{
        let parent = self.items[self.index_from_id(parent_id) as usize].clone();
        let parent_relpos = parent.relpos;
        let grandparent = self.items[self.index_from_id(parent.parent) as usize].clone();
        let mut uncles = grandparent.children.clone();
        let uncle_qty = uncles.len() as u32;
        uncles.sort_unstable_by_key(|topid | self.items[self.index_from_id(*topid) as usize].relpos);

        let mut siblings = parent.children.clone();
        siblings.sort_unstable_by_key(|topid | self.items[self.index_from_id(*topid) as usize].relpos);
        let sibling_qty = siblings.len() as u32;



        // set topic's parent equal to parent's parent 
        update_topic_parent(conn, topic_id, grandparent.id).unwrap();
        // set topic's relpos equal to parent's relpos 
        update_topic_relpos(conn, topic_id, parent_relpos).unwrap();
        // move topic's parent's relpos up one and shift all the relpos of its siblings beneaht it 

        let mut relpos = parent.relpos;
        for i in parent_relpos..uncle_qty{
            relpos += 1;
           update_topic_relpos(conn, uncles[i as usize], relpos).unwrap();
        }

        // move siblings relpos one up 
        
        for i in 1..sibling_qty {
           update_topic_relpos(conn, siblings[i as usize], i - 1).unwrap();
        }
    }else{
        // update id with 
        update_topic_relpos(conn, topic_id, relpos - 1).unwrap();
        let above_index = self.index_sibling_above(index);
        let sibling_above_id = self.items[above_index as usize].id;
        update_topic_relpos(conn, sibling_above_id, relpos).unwrap();

        
    }
}



/// finds distance to sibling above selected topic
pub fn index_sibling_above(&self, index: u32) -> u32{
    let topic = self.items[index as usize].clone();
    if topic.relpos == 0 {return index - 1}
    let topic_id = topic.id;
    let parent = topic.parent;
    let mut siblings = self.items[self.index_from_id(parent) as usize].children.clone();
    siblings.sort_unstable_by_key(|topid | self.items[self.index_from_id(*topid) as usize].relpos);
    let mut sibling_index: usize = 0;
    for (index, sibling) in siblings.iter().enumerate(){
        if *sibling == topic_id{
            sibling_index = index - 1;
            break;
        }
    }
    let id_above = siblings[sibling_index];
    let index_above = self.index_from_id(id_above);
    index_above
}

fn dfs(&mut self, id: u32, indices: &mut Vec<u32>){

    let topic_index = self.index_from_id(id);
    let topic = self.items[topic_index as usize].clone();
    if topic.parent != 0 {
    self.items[topic_index as usize].ancestors =  self.items[self.index_from_id(topic.parent) as usize].ancestors + 1;
}

    let mut kids = topic.children;
    kids.sort_unstable_by_key(|topid | self.items[self.index_from_id(*topid) as usize].relpos);

    for child in kids{
        indices.push(child);
        StatefulList::dfs(self, child, indices);
    }
}


pub fn sort_topics(&mut self){
    let mut ids = vec![1 as u32];

    self.items[0].ancestors = 0;

    StatefulList::dfs(self, 1, &mut ids);

    let mut sorted_topics = Vec::<Topic>::new();

    for id in ids{
        let topic_index = self.index_from_id(id);
        let topic = self.items[topic_index as  usize].clone();
        sorted_topics.push(topic);
    }

    self.items = sorted_topics;

    }
}


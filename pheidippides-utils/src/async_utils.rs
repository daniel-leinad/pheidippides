use tokio::sync::{broadcast, mpsc::{self, UnboundedReceiver, UnboundedSender}};

pub fn pipe_unbounded_channel<I, O, F>(mut channel: mpsc::UnboundedReceiver<I>, mut f: F) -> mpsc::UnboundedReceiver<O> 
where
    I: 'static + Send, 
    O: 'static + Send, 
    F: 'static + FnMut(I) -> Option<O> + Send
{
    let (sender, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = sender.closed() => {
                    // receiver is dropped, drop sender
                    break;
                },

                message_res = channel.recv() => {
                    let message_in = match message_res {
                        Some(message) => message,
                        None => {
                            // previous sender is dropped, drop sender
                            break
                        },
                    };
                    if let Some(message_out) = f(message_in){
                        if let Err(_) = sender.send(message_out) {
                            // receiver is dropped, drop sender
                            break
                        }
                    }
                },
            }
        }
    });
    receiver
}

pub fn pipe_broadcast<I, O, F>(mut in_channel: broadcast::Receiver<I>, mut f: F) -> mpsc::UnboundedReceiver<O> 
where
    I: 'static + Send + Clone, 
    O: 'static + Send, 
    F: 'static + FnMut(I) -> Option<O> + Send
{
    let (sender, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = sender.closed() => {
                    // receiver is dropped, drop sender
                    break;
                },

                message_res = in_channel.recv() => {
                    let message_in = match message_res {
                        Ok(message) => message,
                        Err(_) => {
                            // previous sender is dropped, drop sender
                            break
                        },
                    };
                    if let Some(message_out) = f(message_in) {
                        if let Err(_) = sender.send(message_out) {
                            // receiver is dropped, drop sender
                            break
                        }
                    }
                },
            }
        }
    });
    receiver
}

pub fn redirect_unbounded_channel<T: 'static + Send>(mut from: UnboundedReceiver<T>, to: UnboundedSender<T>) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = to.closed() => {
                    // receiver is dropped, drop sender
                    break;
                },

                message_res = from.recv() => {
                    let message = match message_res {
                        Some(message) => message,
                        None => {
                            // previous sender is dropped, drop sender
                            break
                        },
                    };
                    if let Err(_) = to.send(message) {
                        // receiver is dropped, drop sender
                        break
                    }
                },
            }
        }
    });
}
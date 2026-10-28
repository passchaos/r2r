//! Current-thread reactor composition and cooperative native callback cleanup.

use super::*;

#[tokio::test(flavor = "current_thread")]
async fn current_thread_runtime_drives_ros_services_and_timer_without_manual_spin() {
    let context = RosContext::create().unwrap();
    let mut server = node(&context, "background_server");
    let mut application = node(&context, "background_application");
    let name = format!("/background_service_{}", uuid::Uuid::new_v4().simple());
    let mut service = server
        .create_service::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let client = application
        .create_client::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let mut executor = Executor::new(context.clone());
    let server_id = executor.add_node(server).unwrap();
    let application_id = executor.add_node(application).unwrap();
    let task = executor.spawn().unwrap();
    let ready = client.wait_for_service().unwrap();
    let local = std::rc::Rc::new(std::cell::Cell::new(0));
    let work = async {
        ready.await?;
        tokio::time::sleep(Duration::from_millis(2)).await;
        let response = client
            .request(&AddTwoInts::Request { a: 20, b: 22 })?
            .await?;
        local.set(response.sum);
        Ok::<_, Error>(())
    };
    let serving = async {
        let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        let sum = request.message.a + request.message.b;
        request.respond(AddTwoInts::Response { sum })
    };
    tokio::time::timeout(Duration::from_secs(5), async { futures::try_join!(serving, work) })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(local.get(), 42);
    let mut executor = tokio::time::timeout(Duration::from_secs(5), task.stop())
        .await
        .unwrap()
        .unwrap();
    assert!(context.is_valid());
    assert!(executor
        .remove_node(server_id)
        .unwrap()
        .name()
        .unwrap()
        .starts_with("background_server"));
    assert!(executor
        .remove_node(application_id)
        .unwrap()
        .name()
        .unwrap()
        .starts_with("background_application"));
}

#[tokio::test(flavor = "current_thread")]
async fn inline_callback_panic_is_reported_after_native_cleanup() {
    use crate::{
        lifecycle_msgs::{msg::Transition, srv::ChangeState},
        LifecycleNode,
    };
    let context = RosContext::create().unwrap();
    let name = format!("background_panic_{}", uuid::Uuid::new_v4().simple());
    let managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    managed.on_configure(Box::new(|_| panic!("background callback sentinel")));
    let mut manager = node(&context, "background_panic_manager");
    let client = manager
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::services_default(),
        )
        .unwrap();
    let mut executor = Executor::new(context.clone());
    executor.add_node(managed.into_node()).unwrap();
    executor.add_node(manager).unwrap();
    let task = executor.spawn().unwrap();
    let control = task.handle();
    tokio::time::timeout(Duration::from_secs(5), client.wait_for_service().unwrap())
        .await
        .unwrap()
        .unwrap();
    let response = client
        .request(&ChangeState::Request {
            transition: Transition {
                id: Transition::TRANSITION_CONFIGURE as u8,
                label: Default::default(),
            },
        })
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap(),
        Err(ExecutorTaskError::ThreadPanicked)
    ));
    assert!(tokio::time::timeout(Duration::from_secs(5), response)
        .await
        .unwrap()
        .is_err());
    assert!(!control.cancel().unwrap());
    assert!(context.is_valid());
}

#[tokio::test(flavor = "current_thread")]
async fn tcp_reactor_and_ros_pubsub_services_progress_on_the_same_runtime_thread() {
    use crate::std_msgs::msg::Int64;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    let context = RosContext::create().unwrap();
    let mut server = node(&context, "background_tcp_server");
    let mut application = node(&context, "background_tcp_application");
    let name = format!("/background_tcp_{}", uuid::Uuid::new_v4().simple());
    let mut service = server
        .create_service::<AddTwoInts::Service>(
            &format!("{name}/sum"),
            QosProfile::services_default(),
        )
        .unwrap();
    let client = application
        .create_client::<AddTwoInts::Service>(
            &format!("{name}/sum"),
            QosProfile::services_default(),
        )
        .unwrap();
    let publisher = server
        .create_publisher::<Int64>(&format!("{name}/values"), QosProfile::default())
        .unwrap();
    let mut messages = application
        .subscribe::<Int64>(&format!("{name}/values"), QosProfile::default())
        .unwrap();
    let ready = client.wait_for_service().unwrap();
    let matched = publisher.wait_for_subscribers().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut executor = Executor::new(context.clone());
    executor.add_node(server).unwrap();
    executor.add_node(application).unwrap();
    let task = executor.spawn().unwrap();
    let runtime_thread = thread::current().id();
    let serving = async {
        for _ in 0..16 {
            assert_eq!(thread::current().id(), runtime_thread);
            let request = service.next().await.unwrap();
            let sum = request.message.a + request.message.b;
            request.respond(AddTwoInts::Response { sum }).unwrap();
        }
    };
    let gateway = async {
        ready.await.unwrap();
        matched.await.unwrap();
        let (mut socket, _) = listener.accept().await.unwrap();
        for _ in 0..16 {
            assert_eq!(thread::current().id(), runtime_thread);
            let input = socket.read_i64().await.unwrap();
            let response = client
                .request(&AddTwoInts::Request { a: input, b: 5 })
                .unwrap()
                .await
                .unwrap();
            publisher.publish(&Int64 { data: response.sum }).unwrap();
            let observed = messages.next().await.unwrap();
            assert_eq!(observed.data, input + 5);
            socket.write_i64(observed.data).await.unwrap();
        }
    };
    let connection = async {
        let mut socket = TcpStream::connect(address).await.unwrap();
        for value in -8..8 {
            socket.write_i64(value).await.unwrap();
            assert_eq!(socket.read_i64().await.unwrap(), value + 5);
            assert_eq!(thread::current().id(), runtime_thread);
        }
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        futures::join!(serving, gateway, connection);
    })
    .await
    .unwrap();
    let recovered = tokio::time::timeout(Duration::from_secs(5), task.stop())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.len(), 2);
    assert!(context.is_valid());
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_a_task_does_not_block_runtime_while_inline_callback_is_pending() {
    use crate::{
        lifecycle_msgs::{msg::Transition, srv::ChangeState},
        LifecycleNode,
    };
    let context = RosContext::create().unwrap();
    let name = format!("background_cooperative_{}", uuid::Uuid::new_v4().simple());
    let managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let (entered, observing) = oneshot::channel();
    let mut entered = Some(entered);
    let (release, blocked) = mpsc::channel();
    managed.on_configure(Box::new(move |_| {
        if let Some(entered) = entered.take() {
            entered.send(()).unwrap();
        }
        blocked.recv_timeout(Duration::from_secs(5)).unwrap();
        crate::CallbackReturn::Success
    }));
    let mut manager = node(&context, "background_cooperative_manager");
    let client = manager
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::services_default(),
        )
        .unwrap();
    let mut executor = Executor::new(context.clone());
    executor.add_node(managed.into_node()).unwrap();
    executor.add_node(manager).unwrap();
    let mut task = executor.spawn().unwrap();
    let control = task.handle();
    tokio::time::timeout(Duration::from_secs(5), client.wait_for_service().unwrap())
        .await
        .unwrap()
        .unwrap();
    let response = client
        .request(&ChangeState::Request {
            transition: Transition {
                id: Transition::TRANSITION_CONFIGURE as u8,
                label: Default::default(),
            },
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), observing)
        .await
        .unwrap()
        .unwrap();
    assert!(!task.is_finished());
    let worker = task.thread.take().unwrap();
    drop(task);
    // This timer and the release must run even though the native callback has
    // not returned. A blocking JoinHandle::join in Drop would deadlock here.
    tokio::time::sleep(Duration::from_millis(2)).await;
    assert!(control.is_cancelled());
    release.send(()).unwrap();
    bounded_join(worker);
    assert!(!control.cancel().unwrap());
    assert!(context.is_valid());
    drop(response);
}

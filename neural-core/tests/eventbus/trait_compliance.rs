// EventBus trait compliance tests
// London School TDD: Focus on interactions between objects and contract verification

use super::*;
use mockall::predicate::*;
use tokio_test;

#[cfg(test)]
mod eventbus_trait_tests {
    use super::*;

    #[tokio::test]
    async fn test_eventbus_should_publish_event_when_valid_channel_provided() {
        // Arrange
        let mut mock_bus = MockEventBusImpl::new();
        let test_event = MockEvent::new("test message");
        let channel = "test_channel";

        // Set expectation - London School focuses on the interaction
        mock_bus
            .expect_publish()
            .with(eq(channel), eq(test_event.clone()))
            .times(1)
            .returning(|_, _| Ok(()));

        // Act
        let result = mock_bus.publish(channel, test_event).await;

        // Assert
        assert!(result.is_ok());
        // Mockall will verify the interaction occurred as expected
    }

    #[tokio::test]
    async fn test_eventbus_should_return_error_when_publish_fails() {
        // Arrange
        let mut mock_bus = MockEventBusImpl::new();
        let test_event = MockEvent::new("test message");
        let channel = "invalid_channel";
        let expected_error = EventBusError::ChannelNotFound(channel.to_string());

        mock_bus
            .expect_publish()
            .with(eq(channel), eq(test_event.clone()))
            .times(1)
            .returning(|channel, _| Err(EventBusError::ChannelNotFound(channel.to_string())));

        // Act
        let result = mock_bus.publish(channel, test_event).await;

        // Assert
        assert!(result.is_err());
        match result.unwrap_err() {
            EventBusError::ChannelNotFound(ch) => assert_eq!(ch, channel),
            _ => panic!("Expected ChannelNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_eventbus_should_create_subscriber_when_valid_channel_provided() {
        // Arrange
        let mut mock_bus = MockEventBusImpl::new();
        let mut mock_subscriber = MockEventSubscriberImpl::new();
        let channel = "test_channel";
        let subscriber_id = "sub_123";

        // Set up subscriber mock
        mock_subscriber
            .expect_id()
            .returning(move || subscriber_id);

        // Set up bus mock to return the subscriber
        mock_bus
            .expect_subscribe()
            .with(eq(channel))
            .times(1)
            .returning(move |_| {
                let mut sub = MockEventSubscriberImpl::new();
                sub.expect_id().returning(|| "sub_123");
                Ok(Box::new(sub))
            });

        // Act
        let result = mock_bus.subscribe(channel).await;

        // Assert
        assert!(result.is_ok());
        let subscriber = result.unwrap();
        assert_eq!(subscriber.id(), subscriber_id);
    }

    #[tokio::test]
    async fn test_eventbus_should_return_error_when_subscribe_to_nonexistent_channel() {
        // Arrange
        let mut mock_bus = MockEventBusImpl::new();
        let channel = "nonexistent_channel";

        mock_bus
            .expect_subscribe()
            .with(eq(channel))
            .times(1)
            .returning(|channel| Err(EventBusError::ChannelNotFound(channel.to_string())));

        // Act
        let result = mock_bus.subscribe(channel).await;

        // Assert
        assert!(result.is_err());
        match result.unwrap_err() {
            EventBusError::ChannelNotFound(ch) => assert_eq!(ch, channel),
            _ => panic!("Expected ChannelNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_eventbus_should_unsubscribe_when_valid_channel_and_subscriber_id() {
        // Arrange
        let mut mock_bus = MockEventBusImpl::new();
        let channel = "test_channel";
        let subscriber_id = "sub_123";

        mock_bus
            .expect_unsubscribe()
            .with(eq(channel), eq(subscriber_id))
            .times(1)
            .returning(|_, _| Ok(()));

        // Act
        let result = mock_bus.unsubscribe(channel, subscriber_id).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_eventbus_should_return_error_when_unsubscribe_invalid_subscriber() {
        // Arrange
        let mut mock_bus = MockEventBusImpl::new();
        let channel = "test_channel";
        let subscriber_id = "invalid_sub";

        mock_bus
            .expect_unsubscribe()
            .with(eq(channel), eq(subscriber_id))
            .times(1)
            .returning(|_, sub_id| Err(EventBusError::SubscriberNotFound(sub_id.to_string())));

        // Act
        let result = mock_bus.unsubscribe(channel, subscriber_id).await;

        // Assert
        assert!(result.is_err());
        match result.unwrap_err() {
            EventBusError::SubscriberNotFound(id) => assert_eq!(id, subscriber_id),
            _ => panic!("Expected SubscriberNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_eventbus_should_list_channels_when_requested() {
        // Arrange
        let mut mock_bus = MockEventBusImpl::new();
        let expected_channels = vec!["channel1".to_string(), "channel2".to_string()];

        mock_bus
            .expect_list_channels()
            .times(1)
            .returning(move || Ok(vec!["channel1".to_string(), "channel2".to_string()]));

        // Act
        let result = mock_bus.list_channels().await;

        // Assert
        assert!(result.is_ok());
        let channels = result.unwrap();
        assert_eq!(channels.len(), 2);
        assert!(channels.contains(&"channel1".to_string()));
        assert!(channels.contains(&"channel2".to_string()));
    }

    #[tokio::test]
    async fn test_eventbus_should_return_subscriber_count_when_valid_channel() {
        // Arrange
        let mut mock_bus = MockEventBusImpl::new();
        let channel = "test_channel";
        let expected_count = 3;

        mock_bus
            .expect_channel_subscriber_count()
            .with(eq(channel))
            .times(1)
            .returning(|_| Ok(3));

        // Act
        let result = mock_bus.channel_subscriber_count(channel).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_count);
    }
}

#[cfg(test)]
mod eventsubscriber_trait_tests {
    use super::*;

    #[test]
    fn test_eventsubscriber_should_return_id_when_requested() {
        // Arrange
        let mut mock_subscriber = MockEventSubscriberImpl::new();
        let expected_id = "sub_123";

        mock_subscriber
            .expect_id()
            .times(1)
            .returning(move || expected_id);

        // Act
        let id = mock_subscriber.id();

        // Assert
        assert_eq!(id, expected_id);
    }

    #[tokio::test]
    async fn test_eventsubscriber_should_receive_event_when_available() {
        // Arrange
        let mut mock_subscriber = MockEventSubscriberImpl::new();
        let expected_event = MockEvent::new("test message");

        mock_subscriber
            .expect_receive()
            .times(1)
            .returning(move || Some(MockEvent::new("test message")));

        // Act
        let result = mock_subscriber.receive().await;

        // Assert
        assert!(result.is_some());
        let event = result.unwrap();
        assert_eq!(event.data, "test message");
    }

    #[tokio::test]
    async fn test_eventsubscriber_should_return_none_when_no_events_available() {
        // Arrange
        let mut mock_subscriber = MockEventSubscriberImpl::new();

        mock_subscriber
            .expect_receive()
            .times(1)
            .returning(|| None);

        // Act
        let result = mock_subscriber.receive().await;

        // Assert
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_eventsubscriber_should_close_gracefully_when_requested() {
        // Arrange
        let mut mock_subscriber = MockEventSubscriberImpl::new();

        mock_subscriber
            .expect_close()
            .times(1)
            .returning(|| ());

        // Act & Assert (no panic should occur)
        mock_subscriber.close().await;
    }
}

#[cfg(test)]
mod contract_interaction_tests {
    use super::*;

    #[tokio::test]
    async fn test_eventbus_and_subscriber_should_work_together_in_typical_workflow() {
        // Arrange - London School: Test the conversation between objects
        let mut mock_bus = MockEventBusImpl::new();
        let mut mock_subscriber = MockEventSubscriberImpl::new();
        let channel = "workflow_channel";
        let subscriber_id = "workflow_sub";
        let test_event = MockEvent::new("workflow message");

        // Set up the conversation sequence
        mock_bus
            .expect_subscribe()
            .with(eq(channel))
            .times(1)
            .returning(move |_| {
                let mut sub = MockEventSubscriberImpl::new();
                sub.expect_id().returning(|| "workflow_sub");
                sub.expect_receive().returning(|| Some(MockEvent::new("workflow message")));
                sub.expect_close().returning(|| ());
                Ok(Box::new(sub))
            });

        mock_bus
            .expect_publish()
            .with(eq(channel), always())
            .times(1)
            .returning(|_, _| Ok(()));

        mock_bus
            .expect_unsubscribe()
            .with(eq(channel), eq(subscriber_id))
            .times(1)
            .returning(|_, _| Ok(()));

        // Act - Execute the typical workflow
        let mut subscriber = mock_bus.subscribe(channel).await.unwrap();
        let _publish_result = mock_bus.publish(channel, test_event).await.unwrap();
        let received_event = subscriber.receive().await;
        let _unsubscribe_result = mock_bus.unsubscribe(channel, subscriber_id).await.unwrap();
        subscriber.close().await;

        // Assert
        assert!(received_event.is_some());
        assert_eq!(received_event.unwrap().data, "workflow message");
        // All interactions are verified by mockall
    }
}
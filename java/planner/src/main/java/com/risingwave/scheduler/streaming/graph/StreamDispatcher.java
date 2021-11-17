package com.risingwave.scheduler.streaming.graph;

import com.google.common.collect.ImmutableList;
import com.risingwave.common.exception.PgErrorCode;
import com.risingwave.common.exception.PgException;
import com.risingwave.proto.streaming.plan.Dispatcher;
import java.util.Collections;
import java.util.List;

/** A dispatcher redirects the output messages from an actor to its successors. */
public class StreamDispatcher {
  private final DispatcherType dispatcherType;
  private final ImmutableList<Integer> columns;

  public StreamDispatcher(DispatcherType dispatcherType, List<Integer> columns) {
    this.dispatcherType = dispatcherType;
    this.columns = ImmutableList.copyOf(columns);
  }

  public DispatcherType getDispatcherType() {
    return dispatcherType;
  }

  public ImmutableList<Integer> getDispatcherColumn() {
    return columns;
  }

  public static StreamDispatcher createSimpleDispatcher() {
    return new StreamDispatcher(DispatcherType.SIMPLE, Collections.emptyList());
  }

  public static StreamDispatcher createRoundRobinDispatcher() {
    return new StreamDispatcher(DispatcherType.ROUND_ROBIN, Collections.emptyList());
  }

  public static StreamDispatcher createHashDispatcher(List<Integer> columns) {
    return new StreamDispatcher(DispatcherType.HASH, columns);
  }

  public static StreamDispatcher createBroadcastDispatcher() {
    return new StreamDispatcher(DispatcherType.BROADCAST, Collections.emptyList());
  }

  public static StreamDispatcher createBlackHoleDispatcher() {
    return new StreamDispatcher(DispatcherType.BLACK_HOLE, Collections.emptyList());
  }

  /** The enum of types of dispatchers. */
  public enum DispatcherType {
    SIMPLE("Dispatch data to the downstream actor, assuming there is only one downstream."),
    ROUND_ROBIN("Dispatch data to multiple downstream actors by round robin strategy."),
    HASH("Dispatch data to multiple downstream actors by hash distribution on a certain column."),
    BROADCAST("Dispatch every data chunk to all downstream actors."),
    BLACK_HOLE("Do not dispatch data.");

    private final String description;

    DispatcherType(String description) {
      this.description = description;
    }
  }

  public Dispatcher serialize() {
    Dispatcher.Builder dispatcherBuilder = Dispatcher.newBuilder();
    Dispatcher.DispatcherType type = Dispatcher.DispatcherType.SIMPLE;
    switch (this.dispatcherType) {
      case SIMPLE:
        break;
      case ROUND_ROBIN:
        type = Dispatcher.DispatcherType.ROUND_ROBIN;
        break;
      case HASH:
        type = Dispatcher.DispatcherType.HASH;
        dispatcherBuilder.setColumnIdx(this.columns.get(0));
        break;
      case BROADCAST:
        type = Dispatcher.DispatcherType.BROADCAST;
        break;
      case BLACK_HOLE:
        type = Dispatcher.DispatcherType.BLACKHOLE;
        break;
      default:
        throw new PgException(PgErrorCode.PROTOCOL_VIOLATION, "No such dispatcher type.");
    }
    dispatcherBuilder.setType(type);
    return dispatcherBuilder.build();
  }
}
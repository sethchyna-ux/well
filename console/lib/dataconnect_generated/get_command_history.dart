part of 'generated.dart';

class GetCommandHistoryVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  GetCommandHistoryVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<GetCommandHistoryData> dataDeserializer = (dynamic json)  => GetCommandHistoryData.fromJson(jsonDecode(json));
  Serializer<GetCommandHistoryVariables> varsSerializer = (GetCommandHistoryVariables vars) => jsonEncode(vars.toJson());
  Future<QueryResult<GetCommandHistoryData, GetCommandHistoryVariables>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<GetCommandHistoryData, GetCommandHistoryVariables> ref() {
    GetCommandHistoryVariables vars= GetCommandHistoryVariables(id: id,);
    return _dataConnect.query("GetCommandHistory", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class GetCommandHistoryCommandHistory {
  final String commandString;
  final Timestamp timestamp;
  GetCommandHistoryCommandHistory.fromJson(dynamic json):
  
  commandString = nativeFromJson<String>(json['commandString']),
  timestamp = Timestamp.fromJson(json['timestamp']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetCommandHistoryCommandHistory otherTyped = other as GetCommandHistoryCommandHistory;
    return commandString == otherTyped.commandString && 
    timestamp == otherTyped.timestamp;
    
  }
  @override
  int get hashCode => Object.hashAll([commandString.hashCode, timestamp.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['commandString'] = nativeToJson<String>(commandString);
    json['timestamp'] = timestamp.toJson();
    return json;
  }

  GetCommandHistoryCommandHistory({
    required this.commandString,
    required this.timestamp,
  });
}

@immutable
class GetCommandHistoryData {
  final GetCommandHistoryCommandHistory? commandHistory;
  GetCommandHistoryData.fromJson(dynamic json):
  
  commandHistory = json['commandHistory'] == null ? null : GetCommandHistoryCommandHistory.fromJson(json['commandHistory']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetCommandHistoryData otherTyped = other as GetCommandHistoryData;
    return commandHistory == otherTyped.commandHistory;
    
  }
  @override
  int get hashCode => commandHistory.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (commandHistory != null) {
      json['commandHistory'] = commandHistory!.toJson();
    }
    return json;
  }

  GetCommandHistoryData({
    this.commandHistory,
  });
}

@immutable
class GetCommandHistoryVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  GetCommandHistoryVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetCommandHistoryVariables otherTyped = other as GetCommandHistoryVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  GetCommandHistoryVariables({
    required this.id,
  });
}

